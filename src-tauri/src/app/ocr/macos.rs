//! Vision text recognition.

use std::path::Path;

use objc2::AnyThread;
use objc2::rc::autoreleasepool;
use objc2_foundation::{NSArray, NSDictionary, NSString, NSURL};
use objc2_vision::{VNImageRequestHandler, VNRecognizeTextRequest, VNRequest, VNRequestTextRecognitionLevel};

pub fn recognize(path: &Path) -> Result<String, super::Unavailable> {
    // Worker threads have no run loop to drain autoreleased objects.
    autoreleasepool(|_| {
        let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
        let handler =
            unsafe { VNImageRequestHandler::initWithURL_options(VNImageRequestHandler::alloc(), &url, &NSDictionary::new()) };
        let request = VNRecognizeTextRequest::new();
        request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
        request.setRecognitionLanguages(&NSArray::from_retained_slice(&[
            NSString::from_str("ko-KR"),
            NSString::from_str("en-US"),
        ]));
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
