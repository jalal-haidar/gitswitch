import { useCallback } from "react";
import { useToast } from "./useToast";
import { normalizeBackendError } from "../../utils/error";

/** Returns a function that shows any thrown backend error as an error toast. */
export function useErrorToast() {
  const { show } = useToast();
  return useCallback(
    (e: unknown, prefix?: string) => {
      const info = normalizeBackendError(e);
      show({
        message: prefix ? `${prefix}: ${info.message}` : info.message,
        kind: "error",
        duration: info.hint ? 8000 : 6000,
      });
    },
    [show],
  );
}
