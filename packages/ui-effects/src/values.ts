import type { ParamValue } from "@videocut/model";

export type DisplayScale = "number" | "percent" | "degree";

export function cloneValue(value: ParamValue): ParamValue {
  switch (value.type) {
    case "float":
    case "int":
    case "bool":
    case "str":
      return { ...value };
    case "color":
      return { type: "color", value: { ...value.value } };
    case "point":
      return { type: "point", value: { ...value.value } };
    default: {
      const never: never = value;
      return never;
    }
  }
}

export function toDisplay(scale: DisplayScale, value: number): number {
  return scale === "percent" ? value * 100 : value;
}

export function fromDisplay(scale: DisplayScale, value: number): number {
  return scale === "percent" ? value / 100 : value;
}
