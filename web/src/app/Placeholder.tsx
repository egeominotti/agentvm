// A screen not moved to the new dashboard yet: it links to the one at /.
export function Placeholder({ what }: { what: string }) {
  return (
    <div className="placeholder">
      <p>
        {what} is still in the current dashboard. <a href={`/${location.hash}`}>Open it there</a>.
      </p>
    </div>
  );
}
