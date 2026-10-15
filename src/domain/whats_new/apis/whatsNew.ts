import type { WhatsNew } from "@/domain/whats_new/apis/whatsNew.dto";
import call from "@/shared/api";

/** The release notes to show once after an update; null when there's nothing to show. */
export async function getWhatsNew() {
  return call<WhatsNew | null>("get_whats_new");
}

/** Forgets the notes and hides the window. */
export async function dismissWhatsNew() {
  return call<void>("dismiss_whats_new");
}
