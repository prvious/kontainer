import { useSyncExternalStore } from "react";

/**
 * One shared one-second clock for relative times, countdowns, and "synced 12s ago" lines,
 * so components never read `Date.now()` during render or start timers in effects.
 */
const TICK_MS = 1000;
const listeners = new Set<() => void>();
let now = Date.now();
let timer: ReturnType<typeof setInterval> | undefined;

function tick() {
    now = Date.now();
    for (const listener of listeners) listener();
}

function subscribe(listener: () => void) {
    listeners.add(listener);
    if (timer === undefined) {
        timer = setInterval(tick, TICK_MS);
        // The first subscriber may mount long after the module loaded.
        tick();
    }
    return () => {
        listeners.delete(listener);
        if (listeners.size === 0 && timer !== undefined) {
            clearInterval(timer);
            timer = undefined;
        }
    };
}

export function useNow(): number {
    return useSyncExternalStore(subscribe, () => now);
}
