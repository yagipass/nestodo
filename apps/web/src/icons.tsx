import type { ReactNode } from "react";

const SHAPES = {
  filter: (
    <>
      <circle cx="10.5" cy="10.5" r="6.5" />
      <path d="M15.5 15.5 20 20" />
    </>
  ),
  completed: (
    <>
      <rect x="3.5" y="3.5" width="17" height="17" rx="5" />
      <path d="m8.5 12.5 2.5 2.5 4.5-5" />
    </>
  ),
  account: (
    <>
      <circle cx="12" cy="8.5" r="3.5" />
      <path d="M5.5 19.5c1.2-3 3.7-4.5 6.5-4.5s5.3 1.5 6.5 4.5" />
    </>
  ),
  stats: <path d="M5.5 19.5v-5M12 19.5v-9M18.5 19.5v-14" />,
  settings: (
    <>
      <path d="M4 8h16M4 16h16" />
      <circle cx="15" cy="8" r="2" fill="currentColor" />
      <circle cx="9" cy="16" r="2" fill="currentColor" />
    </>
  ),
  help: (
    <>
      <rect x="3.5" y="3.5" width="17" height="17" rx="5" />
      <path d="M9.8 9.6a2.3 2.3 0 1 1 3.1 2.1c-.6.3-.9.8-.9 1.4v.3" />
      <circle cx="12" cy="16.4" r=".6" fill="currentColor" />
    </>
  ),
  signOut: <path d="M10 4H7.5A3.5 3.5 0 0 0 4 7.5v9A3.5 3.5 0 0 0 7.5 20H10M15 8l4 4-4 4M19 12h-9" />,
  expand: <path d="M9.5 6.5 15 12l-5.5 5.5" />,
  note: (
    <>
      <rect x="4.5" y="3.5" width="15" height="17" rx="4" />
      <circle cx="9" cy="9" r=".75" fill="currentColor" />
      <path d="M12.5 9h3M9 13.5h6.5M9 17h3.5" />
    </>
  ),
  done: <rect x="4" y="4" width="16" height="16" rx="5" />,
  link: <path d="M8 16 16 8M10 8h6v6" />,
  add: <path d="M12 5.5v13M5.5 12h13" />,
  close: <path d="M7 7l10 10M17 7 7 17" />,
  tick: <path d="M5 12l5 5 9-10" />,
} satisfies Record<string, ReactNode>;

export type IconName = keyof typeof SHAPES;

export function Icon({ name, size, strokeWidth = 2 }: { name: IconName; size: number; strokeWidth?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={strokeWidth}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      {SHAPES[name]}
    </svg>
  );
}
