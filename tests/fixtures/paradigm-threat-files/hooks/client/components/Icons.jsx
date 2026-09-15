import React from 'react'

function Icon(props) {
  const size = props.size
  const children = props.children
  return (
    <svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor"
      style={{ display: 'inline-block', flexShrink: 0, strokeWidth: 2, strokeLinecap: 'round', strokeLinejoin: 'round' }}>
      {children}
    </svg>
  )
}

export function IconBook() {
  return (
    <Icon size={18}>
      <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20" />
      <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z" />
    </Icon>
  )
}

export function IconShare() {
  return (
    <Icon size={18}>
      <circle cx="18" cy="5" r="3" />
      <circle cx="6" cy="12" r="3" />
      <circle cx="18" cy="19" r="3" />
      <line x1="8.59" x2="15.42" y1="13.51" y2="17.49" />
      <line x1="15.41" x2="8.59" y1="6.51" y2="10.49" />
    </Icon>
  )
}

export function IconSearch() {
  return (
    <Icon size={18}>
      <circle cx="11" cy="11" r="8" />
      <path d="m21 21-4.3-4.3" />
    </Icon>
  )
}

export function IconHome() {
  return (
    <Icon size={18}>
      <path d="m3 9 9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
      <path d="M9 22V12h6v10" />
    </Icon>
  )
}

export function IconMenu() {
  return (
    <Icon size={24}>
      <line x1="4" x2="20" y1="12" y2="12" />
      <line x1="4" x2="20" y1="6" y2="6" />
      <line x1="4" x2="20" y1="18" y2="18" />
    </Icon>
  )
}

export function IconClose() {
  return (
    <Icon size={24}>
      <path d="M18 6 6 18" />
      <path d="m6 6 12 12" />
    </Icon>
  )
}

export function IconSun() {
  return (
    <Icon size={20}>
      <circle cx="12" cy="12" r="4" />
      <path d="M12 2v2" />
      <path d="M12 20v2" />
      <path d="m4.93 4.93 1.41 1.41" />
      <path d="m17.66 17.66 1.41 1.41" />
      <path d="M2 12h2" />
      <path d="M20 12h2" />
      <path d="m6.34 17.66-1.41 1.41" />
      <path d="m19.07 4.93-1.41 1.41" />
    </Icon>
  )
}

export function IconMoon() {
  return (
    <Icon size={20}>
      <path d="M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z" />
    </Icon>
  )
}

export function IconClock() {
  return (
    <Icon size={18}>
      <circle cx="12" cy="12" r="10" />
      <path d="M12 6v6l4 2" />
    </Icon>
  )
}

export function IconMessage() {
  return (
    <Icon size={18}>
      <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
    </Icon>
  )
}

export function IconVideo() {
  return (
    <Icon size={18}>
      <path d="m16 13 5.223 3.482a.5.5 0 0 0 .777-.416V7.87a.5.5 0 0 0-.752-.432L16 10.5" />
      <rect x="2" y="6" width="14" height="12" rx="2" />
    </Icon>
  )
}

export function IconHeart() {
  return (
    <Icon size={18}>
      <path d="M19 14c1.49-1.46 3-3.21 3-5.5A5.5 5.5 0 0 0 16.5 3c-1.76 0-3 .5-4.5 2-1.5-1.5-2.74-2-4.5-2A5.5 5.5 0 0 0 2 8.5c0 2.3 1.5 4.05 3 5.5l7 7Z" />
    </Icon>
  )
}

export function IconRss() {
  return (
    <Icon size={18}>
      <path d="M4 11a9 9 0 0 1 9 9" />
      <path d="M4 4a16 16 0 0 1 16 16" />
      <circle cx="5" cy="19" r="1" />
    </Icon>
  )
}

export function IconMaximize() {
  return (
    <Icon size={48}>
      <path d="M8 3H5a2 2 0 0 0-2 2v3" />
      <path d="M21 8V5a2 2 0 0 0-2-2h-3" />
      <path d="M3 16v3a2 2 0 0 0 2 2h3" />
      <path d="M16 21h3a2 2 0 0 0 2-2v-3" />
    </Icon>
  )
}

export function IconChevronLeft() {
  return (
    <Icon size={32}>
      <path d="m15 18-6-6 6-6" />
    </Icon>
  )
}

export function IconChevronRight() {
  return (
    <Icon size={32}>
      <path d="m9 18 6-6-6-6" />
    </Icon>
  )
}

export function IconChevronUp() {
  return (
    <Icon size={20}>
      <path d="m18 15-6-6-6 6" />
    </Icon>
  )
}

export function IconChevronDownSm() {
  return (
    <Icon size={14}>
      <path d="m6 9 6 6 6-6" />
    </Icon>
  )
}

export function IconChevronRightSm() {
  return (
    <Icon size={14}>
      <path d="m9 18 6-6-6-6" />
    </Icon>
  )
}

export function IconFileSm() {
  return (
    <Icon size={12}>
      <path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z" />
      <path d="M14 2v6h6" />
    </Icon>
  )
}

/** Filled play triangle (TTS dock — matches old site lucide Play). */
export function IconPlay() {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      width={24}
      height={24}
      viewBox="0 0 24 24"
      aria-hidden="true"
      style={{ display: 'inline-block', flexShrink: 0, marginLeft: '0.15rem' }}
    >
      <polygon points="6 3 20 12 6 21 6 3" fill="currentColor" />
    </svg>
  )
}

export function IconPause() {
  return (
    <Icon size={24}>
      <rect x="6" y="4" width="4" height="16" rx="1" />
      <rect x="14" y="4" width="4" height="16" rx="1" />
    </Icon>
  )
}

export function IconStop() {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      width={18}
      height={18}
      viewBox="0 0 24 24"
      aria-hidden="true"
      style={{ display: 'inline-block', flexShrink: 0 }}
    >
      <rect x="5" y="5" width="14" height="14" rx="1" fill="currentColor" />
    </svg>
  )
}

/** Lucide Settings2-style sliders (TTS dock). */
export function IconSettings() {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      width={18}
      height={18}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={2}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      style={{ display: 'inline-block', flexShrink: 0 }}
    >
      <path d="M20 7h-9" />
      <path d="M14 17H5" />
      <circle cx="17" cy="17" r="3" />
      <circle cx="7" cy="7" r="3" />
    </svg>
  )
}
