import type { AppError } from "../shared/api/common.dto.ts";

/** Text for a failed command: an `AppError`'s message, a string as is, anything else stringified. */
export function errorText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object" && "message" in e) return (e as AppError).message;
  return String(e);
}
