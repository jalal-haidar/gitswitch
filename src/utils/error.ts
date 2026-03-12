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
        // Try direct parse first
        const parsed = JSON.parse(e) as BackendError;
        const title = parsed.kind || "Error";
        // If backend returned a generic message like "Git command failed", prefer details when available
        const message =
          parsed.message && parsed.message !== "Git command failed"
            ? parsed.message
            : parsed.details || "An error occurred";
        return {
          title,
          message,
          hint: parsed.hint,
          details: parsed.details,
          kind: parsed.kind,
        };
      } catch (_) {
        // Try to extract JSON substring from strings like "Error: {...}" or wrapped values
        const jsonMatch = e.match(/(\{[\s\S]*\})/);
        if (jsonMatch) {
          try {
            const parsed = JSON.parse(jsonMatch[1]) as BackendError;
            const title = parsed.kind || "Error";
            const message =
              parsed.message && parsed.message !== "Git command failed"
                ? parsed.message
                : parsed.details || "An error occurred";
            return {
              title,
              message,
              hint: parsed.hint,
              details: parsed.details,
              kind: parsed.kind,
            };
          } catch {
            // fallthrough to plain string
          }
        }
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
