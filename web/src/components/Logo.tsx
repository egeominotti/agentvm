// agentvm's mark: four corner brackets around a sealed cell, the agent inside. The brackets take
// the text color, so the mark reads in light and dark mode alike.
export function Logo({ size = 18, className }: { size?: number; className?: string }) {
  return (
    <svg className={className} width={size} height={size} viewBox="0 0 64 64" fill="none" aria-hidden="true">
      <g stroke="currentColor" strokeWidth="5" strokeLinecap="square">
        <path d="M6 20V6h14" />
        <path d="M44 6h14v14" />
        <path d="M58 44v14H44" />
        <path d="M20 58H6V44" />
      </g>
      <rect x="22" y="22" width="12" height="20" fill="#A08CFF" />
      <rect x="38" y="36" width="6" height="6" fill="currentColor" opacity=".5" />
    </svg>
  );
}
