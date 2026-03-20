export interface FontDefinition {
    id: string;
    label: string;
    cssFamily: string;
    load: () => Promise<void>;
}

export const DEFAULT_FONT_ID = "space-grotesk";

export const fonts: FontDefinition[] = [
    {
        id: "space-grotesk",
        label: "Space Grotesk",
        cssFamily: '"Space Grotesk Variable", sans-serif',
        load: () => Promise.resolve(),
    },
    {
        id: "figtree",
        label: "Figtree",
        cssFamily: '"Figtree Variable", sans-serif',
        load: () => Promise.resolve(),
    },
    {
        id: "inter",
        label: "Inter",
        cssFamily: '"Inter Variable", sans-serif',
        load: () => import("@fontsource-variable/inter").then(() => {}),
    },
    {
        id: "plus-jakarta-sans",
        label: "Plus Jakarta Sans",
        cssFamily: '"Plus Jakarta Sans Variable", sans-serif',
        load: () => import("@fontsource-variable/plus-jakarta-sans").then(() => {}),
    },
    {
        id: "dm-sans",
        label: "DM Sans",
        cssFamily: '"DM Sans Variable", sans-serif',
        load: () => import("@fontsource-variable/dm-sans").then(() => {}),
    },
    {
        id: "outfit",
        label: "Outfit",
        cssFamily: '"Outfit Variable", sans-serif',
        load: () => import("@fontsource-variable/outfit").then(() => {}),
    },
    {
        id: "nunito-sans",
        label: "Nunito Sans",
        cssFamily: '"Nunito Sans Variable", sans-serif',
        load: () => import("@fontsource-variable/nunito-sans").then(() => {}),
    },
];

export function getFontById(id: string): FontDefinition {
    return fonts.find((f) => f.id === id) ?? fonts[0];
}
