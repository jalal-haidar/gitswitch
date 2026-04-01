import { useEffect, useState } from "react";

/**
 * Debounce a value by a specified delay.
 * Returns the debounced value which only updates after
 * the delay has passed since the last value change.
 */
export function useDebounce<T>(value: T, delay: number = 300): T {
  const [debouncedValue, setDebouncedValue] = useState<T>(value);

  useEffect(() => {
    const handler = setTimeout(() => {
      setDebouncedValue(value);
    }, delay);

    return () => {
      clearTimeout(handler);
    };
  }, [value, delay]);

  return debouncedValue;
}
