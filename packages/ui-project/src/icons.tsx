import type { ReactNode, SVGProps } from "react";

type IconProps = SVGProps<SVGSVGElement>;

function Icon({ children, ...props }: IconProps & { children: ReactNode }) {
  return (
    <svg
      viewBox="0 0 16 16"
      width="16"
      height="16"
      aria-hidden="true"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.2"
      strokeLinejoin="round"
      {...props}
    >
      {children}
    </svg>
  );
}

export function FolderIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M1.5 4.2h4.1l1.1-1.4h7.8V13H1.5z" />
    </Icon>
  );
}

export function ClipIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="2" y="3" width="12" height="10" rx="1" />
      <path d="M5 3v10M11 3v10M2 6.2h3M11 6.2h3M2 9.8h3M11 9.8h3" />
    </Icon>
  );
}

export function SequenceIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="3.2" y="3.2" width="9" height="7" />
      <path d="M5 12.8h8.2V5.6" />
    </Icon>
  );
}

export function ListIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M2 4h12M2 8h12M2 12h12" />
    </Icon>
  );
}

export function GridIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="2" y="2" width="5" height="5" />
      <rect x="9" y="2" width="5" height="5" />
      <rect x="2" y="9" width="5" height="5" />
      <rect x="9" y="9" width="5" height="5" />
    </Icon>
  );
}
