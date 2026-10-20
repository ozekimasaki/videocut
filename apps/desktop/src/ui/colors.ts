const LABEL_COLORS = ["#5c6b7a", "#c45c4a", "#d08a3e", "#c6b15a", "#4f9a6e", "#3d8f8a", "#4d78a8", "#7a6aa8"];

export function labelColor(index: number): string {
  return LABEL_COLORS[Math.abs(index) % LABEL_COLORS.length] ?? LABEL_COLORS[0]!;
}
