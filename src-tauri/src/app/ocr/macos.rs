//! Vision text recognition.

use std::path::Path;

use objc2::AnyThread;
use objc2::rc::autoreleasepool;
use objc2::runtime::AnyClass;
use objc2_foundation::{NSArray, NSDictionary, NSString, NSURL};
use objc2_vision::{VNImageRequestHandler, VNRecognizeTextRequest, VNRequest, VNRequestTextRecognitionLevel};

use super::Unavailable;

const LANGUAGES: [&str; 2] = ["ko-KR", "en-US"];

/// The wanted languages the OS supports, in our order. `None` when it supports none of them.
fn pick_languages(supported: &[String]) -> Option<Vec<&'static str>> {
    let picked: Vec<_> = LANGUAGES.into_iter().filter(|l| supported.iter().any(|s| s == l)).collect();
    (!picked.is_empty()).then_some(picked)
}

pub fn recognize(path: &Path) -> Result<String, Unavailable> {
    // Worker threads have no run loop to drain autoreleased objects.
    autoreleasepool(|_| {
        // Text recognition needs macOS 10.15; creating the request on older systems would panic.
        AnyClass::get(c"VNRecognizeTextRequest").ok_or(Unavailable)?;
        let request = VNRecognizeTextRequest::new();
        request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
        // macOS 12 and older have no Korean; ask only for what this OS supports. The class method
        // is deprecated but, unlike the instance one (macOS 12+), exists since 10.15.
        #[allow(deprecated)]
        let supported = unsafe {
            VNRecognizeTextRequest::supportedRecognitionLanguagesForTextRecognitionLevel_revision_error(
                VNRequestTextRecognitionLevel::Accurate,
                request.revision(),
            )
        }
        .map_err(|_| Unavailable)?;
        let supported: Vec<String> = supported.iter().map(|s| s.to_string()).collect();
        let languages = pick_languages(&supported).ok_or(Unavailable)?;
        request.setRecognitionLanguages(&NSArray::from_retained_slice(
            &languages.iter().map(|l| NSString::from_str(l)).collect::<Vec<_>>(),
        ));

        let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
        let handler =
            unsafe { VNImageRequestHandler::initWithURL_options(VNImageRequestHandler::alloc(), &url, &NSDictionary::new()) };
        let requests = NSArray::<VNRequest>::from_slice(&[request.as_ref()]);
        // Missing, unreadable, or non-image files fail here; they just have no text.
        if handler.performRequests_error(&requests).is_err() {
            return Ok(String::new());
        }
        let lines: Vec<String> = request
            .results()
            .map(|observations| {
                observations
                    .iter()
                    .filter_map(|o| o.topCandidates(1).firstObject().map(|t| t.string().to_string()))
                    .collect()
            })
            .unwrap_or_default();
        Ok(lines.join("\n"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn langs(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn picks_only_supported_languages_in_our_order() {
        assert_eq!(pick_languages(&langs(&["en-US"])), Some(vec!["en-US"]));
        assert_eq!(pick_languages(&langs(&["fr-FR", "en-US", "ko-KR"])), Some(vec!["ko-KR", "en-US"]));
        assert_eq!(pick_languages(&langs(&["fr-FR"])), None);
        assert_eq!(pick_languages(&[]), None);
    }
}
