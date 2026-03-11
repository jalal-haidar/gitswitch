export type BackendError = {
  kind: string;
  message: string;
  hint?: string;
  details?: string;
};

export function normalizeBackendError(e: unknown) {
  // Try parse JSON structured error sent from backend
  const fallback = {
    title: "Error",
    message:
      e && typeof e === "object" && "toString" in e
        ? String(e)
        : "An error occurred",
  };

  try {
    if (typeof e === "string") {
      // backend often returns serialized JSON string for structured errors
      try {
        const parsed = JSON.parse(e) as BackendError;
        const title = parsed.kind || "Error";
        const message = parsed.message || parsed.details || "An error occurred";
        return {
          title,
          message,
          hint: parsed.hint,
          details: parsed.details,
          kind: parsed.kind,
        };
      } catch (_) {
        // not JSON — fallthrough
        return { title: "Error", message: e };
      }
    }

    if (e && typeof e === "object") {
      const obj: any = e as any;
      if (obj.kind && obj.message) {
        return {
          title: obj.kind,
          message: obj.message,
          hint: obj.hint,
          details: obj.details,
          kind: obj.kind,
        };
      }
    }

    return fallback;
  } catch (err) {
    return fallback;
  }
}
