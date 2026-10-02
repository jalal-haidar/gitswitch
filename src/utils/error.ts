export type BackendError = {
  kind: string;
  message: string;
  hint?: string;
  details?: string;
};

export interface NormalizedError {
  title: string;
  message: string;
  hint?: string;
  details?: string;
  kind?: string;
}

const GENERIC_MESSAGES = new Set(["Git command failed", "SSH command failed"]);

function fromStructured(parsed: BackendError): NormalizedError {
  // Generic wrapper messages carry the useful text in `details`.
  const message =
    parsed.message && !GENERIC_MESSAGES.has(parsed.message)
      ? parsed.message
      : parsed.details || parsed.message || "An error occurred";
  return {
    title: parsed.kind || "Error",
    message,
    hint: parsed.hint ?? undefined,
    details: parsed.details ?? undefined,
    kind: parsed.kind,
  };
}

function isStructured(v: unknown): v is BackendError {
  return (
    !!v &&
    typeof v === "object" &&
    typeof (v as BackendError).kind === "string" &&
    typeof (v as BackendError).message === "string"
  );
}

function parseStructuredString(s: string): BackendError | null {
  // Backends may send raw JSON, or JSON wrapped in text like "Error: {...}".
  const candidates = [s];
  const match = s.match(/(\{[\s\S]*\})/);
  if (match) candidates.push(match[1]);
  for (const c of candidates) {
    try {
      const parsed = JSON.parse(c);
      if (isStructured(parsed)) return parsed;
    } catch {
      // try next candidate
    }
  }
  return null;
}

/** Accepts anything thrown by `invoke` (structured object, JSON string, plain string, Error). */
export function normalizeBackendError(e: unknown): NormalizedError {
  if (isStructured(e)) return fromStructured(e);

  if (typeof e === "string") {
    const parsed = parseStructuredString(e);
    return parsed ? fromStructured(parsed) : { title: "Error", message: e };
  }

  if (e instanceof Error) return { title: "Error", message: e.message };

  return { title: "Error", message: "An error occurred" };
}

/** One-line message suitable for a toast or inline error. */
export function toUserMessage(e: unknown): string {
  return normalizeBackendError(e).message;
}
