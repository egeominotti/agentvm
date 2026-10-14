// Saves finish independently of typing: only the current edit can be reported as saved, and
// another write waits until the previous one has finished so responses cannot reorder settings.
export class PendingSaves {
  private revision = 0;
  private active: number | null = null;
  private requested = false;

  edit() {
    this.revision++;
  }

  start(): number | null {
    if (this.active !== null) {
      this.requested = true;
      return null;
    }
    this.requested = false;
    this.active = this.revision;
    return this.active;
  }

  isCurrent(revision: number): boolean {
    return revision === this.revision;
  }

  /** Whether a write requested while this one ran must now be sent. */
  finish(revision: number): boolean {
    if (this.active !== revision) return false;
    this.active = null;
    return this.requested;
  }
}
