/**
 * The shared IPC fixtures, typed as the views React reads (ADR 0032).
 *
 * The Rust tests assert that each view serializes to the same JSON file, so a fixture that no
 * longer fits its TypeScript view fails `pnpm types` whichever side drifted. JSON imports widen
 * string literals to `string`, so a view's string unions are compared as `string`: a renamed,
 * missing, or retyped field still fails.
 */
export type Wire<T> = T extends string
    ? string
    : T extends readonly (infer Item)[]
      ? Wire<Item>[]
      : T extends object
        ? string extends keyof T
            ? // A map such as labels: TypeScript gives an empty `{}` beside a filled one optional `undefined` keys.
              Record<string, Wire<T[keyof T]> | undefined>
            : { [Key in keyof T]: Wire<T[Key]> }
        : T;

/** The fixture as its view, once the compiler has checked it fits. */
export const ipcFixture = <T>(fixture: Wire<NoInfer<T>>): T => structuredClone(fixture) as T;
