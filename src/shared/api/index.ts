import type { AppError } from "@/shared/api/common.dto";

import { type InvokeArgs, invoke } from "@tauri-apps/api/core";

function toAppError(e: unknown): AppError {
  if (e && typeof e === "object" && "code" in e && "message" in e) {
    return e as AppError;
  }

  return { code: "UNKNOWN", message: String(e) };
}

/** The only place that calls Tauri `invoke`. Rejects with `AppError`. */
export default async function call<T>(cmd: string, args?: InvokeArgs): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    throw toAppError(e);
  }
}
