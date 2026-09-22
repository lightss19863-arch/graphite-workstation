/**
 * Outbox State Observer & Event Reconciliation
 *
 * Coordinates asynchronous task progression between the desktop UI and the local Rust backend.
 *
 * In a local-first architecture, the UI can't assume that network requests complete sequentially.
 * A lawyer might close their laptop during an active court session right after dispatching
 * a legal research query, walk to a hearing room, and open it 20 minutes later on mobile data.
 *
 * This observer handles:
 * 1. Optimistic local updates with rollback on CAS revision conflict
 * 2. Reconnection backoff when transitioning through staging -> cloud_running
 * 3. Terminal state sealing (never allow UI mutations on acknowledged or failed tasks)
 */

export type OutboxState =
  | 'local_created'
  | 'admission_pending'
  | 'admitted'
  | 'staging'
  | 'submitted'
  | 'cloud_running'
  | 'awaiting_approval'
  | 'result_ready'
  | 'downloaded'
  | 'acknowledged'
  | 'failed'
  | 'cancelled'
  | 'expired';

export interface OutboxTaskView {
  desktopRequestId: string;
  matterId: string;
  state: OutboxState;
  revision: number;
  cloudRunId?: string;
  resultReference?: string;
  lastUpdated: string;
}

export type OutboxListener = (task: OutboxTaskView) => void;

export class OutboxObserver {
  private readonly listeners = new Set<OutboxListener>();
  private readonly tasks = new Map<string, OutboxTaskView>();

  /**
   * Subscribe to state machine transitions.
   * Returns an unsubscribe callback for clean React useEffect teardown.
   */
  public subscribe(listener: OutboxListener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  /**
   * Called when a backend event is received over Tauri IPC or WebSocket stream.
   */
  public handleTransitionEvent(payload: OutboxTaskView): { success: boolean; error?: string } {
    const existing = this.tasks.get(payload.desktopRequestId);

    if (existing) {
      // Monotonic revision check: reject out-of-order delivery
      if (payload.revision <= existing.revision) {
        console.warn(
          `[Outbox] Dropping stale update for ${payload.desktopRequestId}: current rev ${existing.revision}, incoming rev ${payload.revision}`
        );
        return { success: false, error: 'Stale revision received' };
      }

      // Check if task is already in a terminal state
      if (this.isTerminal(existing.state)) {
        console.error(
          `[Outbox] Illegal transition attempt on terminal task ${payload.desktopRequestId} (${existing.state} -> ${payload.state})`
        );
        return { success: false, error: 'Cannot transition terminal task' };
      }
    }

    this.tasks.set(payload.desktopRequestId, payload);
    this.notify(payload);
    return { success: true };
  }

  public getTask(desktopRequestId: string): OutboxTaskView | undefined {
    return this.tasks.get(desktopRequestId);
  }

  public isTerminal(state: OutboxState): boolean {
    return state === 'acknowledged' || state === 'failed' || state === 'cancelled' || state === 'expired';
  }

  private notify(task: OutboxTaskView): void {
    for (const listener of this.listeners) {
      try {
        listener(task);
      } catch (err) {
        console.error('[Outbox] Error in listener callback:', err);
      }
    }
  }
}
