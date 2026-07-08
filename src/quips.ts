// Shown while connecting. This is the one sanctioned home of the seance
// gimmick (plus the palette)
export const QUIPS = [
  "Holding a seance...",
  "Consulting the shadows...",
  "Shining the crystal ball...",
  "Lighting the candles...",
  "Drawing back the veil...",
  "Knocking twice for yes...",
  "Reading the tea leaves...",
  "Listening for whispers...",
];

export function randomQuip(exclude?: string): string {
  const pool = QUIPS.filter((q) => q !== exclude);
  return pool[Math.floor(Math.random() * pool.length)];
}
