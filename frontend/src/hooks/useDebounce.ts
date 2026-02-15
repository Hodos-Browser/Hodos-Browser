import { useRef, useEffect, useMemo } from 'react';

/**
 * Debounces a callback function to delay execution until after a specified delay.
 * Uses useRef to avoid stale closures with React state.
 */
export function useDebounce<T extends (...args: any[]) => any>(
  callback: T,
  delay: number
): (...args: Parameters<T>) => void {
  // Store latest callback in ref (updated every render)
  const callbackRef = useRef<T>(callback);

  useEffect(() => {
    callbackRef.current = callback;
  }, [callback]);

  // Create debounced function once on mount
  const debouncedCallback = useMemo(() => {
    let timeoutId: ReturnType<typeof setTimeout> | undefined;

    return (...args: Parameters<T>) => {
      // Clear previous timeout
      if (timeoutId !== undefined) {
        clearTimeout(timeoutId);
      }

      // Schedule new execution
      timeoutId = setTimeout(() => {
        callbackRef.current(...args);
      }, delay);
    };
  }, [delay]);

  return debouncedCallback;
}
