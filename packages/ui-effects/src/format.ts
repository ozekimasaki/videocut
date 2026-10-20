import type { ParamValue } from "@videocut/model";

const PERCENT_IDS = new Set(["opacity", "left", "top", "right", "bottom"]);
const DEGREE_IDS = new Set(["rotation", "skew", "skewAxis"]);

export function formatParamValue(id: string, value: ParamValue): string {
  switch (value.type) {
    case "float":
      if (PERCENT_IDS.has(id)) return `${(value.value * 100).toFixed(1)}%`;
      if (DEGREE_IDS.has(id)) return `${value.value.toFixed(2)}°`;
      return value.value.toFixed(3);
    case "int":
      return String(value.value);
    case "bool":
      return value.value ? "On" : "Off";
    case "point":
      if (id === "scale") {
        return `${(value.value.x * 100).toFixed(1)}%, ${(value.value.y * 100).toFixed(1)}%`;
      }
      return `${value.value.x.toFixed(3)}, ${value.value.y.toFixed(3)}`;
    case "color":
      return `${value.value.r.toFixed(2)}, ${value.value.g.toFixed(2)}, ${value.value.b.toFixed(2)}, ${value.value.a.toFixed(2)}`;
    case "str":
      return value.value;
    default: {
      const never: never = value;
      return never;
    }
  }
}

export function formatBlend(amount: number): string {
  return `${(amount * 100).toFixed(1)}%`;
}
