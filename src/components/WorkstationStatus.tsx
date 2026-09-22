import React, { useEffect, useState } from 'react';
import { OutboxObserver, type OutboxTaskView } from '../outbox-observer';
import './WorkstationStatus.css';

interface WorkstationStatusProps {
  observer: OutboxObserver;
  activeRequestId?: string;
  vaultStatus?: 'locked' | 'unlocked' | 'hardware_sealed';
  dbStatus?: 'connected' | 'encrypting' | 'synced';
}

export const WorkstationStatus: React.FC<WorkstationStatusProps> = ({
  observer,
  activeRequestId,
  vaultStatus = 'hardware_sealed',
  dbStatus = 'synced',
}) => {
  const [currentTask, setCurrentTask] = useState<OutboxTaskView | null>(null);

  useEffect(() => {
    if (!activeRequestId) return;

    const initial = observer.getTask(activeRequestId);
    if (initial) {
      setCurrentTask(initial);
    }

    const unsubscribe = observer.subscribe((task) => {
      if (task.desktopRequestId === activeRequestId) {
        setCurrentTask(task);
      }
    });

    return unsubscribe;
  }, [observer, activeRequestId]);

  return (
    <aside className="workstation-status-panel" aria-label="Security and Sync Status">
      <header className="status-header">
        <span className="status-title">Runtime Integrity</span>
        <span className={`status-badge badge-${vaultStatus}`}>
          {vaultStatus === 'hardware_sealed' ? 'Device Sealed' : vaultStatus}
        </span>
      </header>

      <section className="status-indicators">
        <div className="status-row">
          <span className="status-label">Storage Engine</span>
          <span className="status-val status-encrypted">SQLCipher AES-256</span>
        </div>
        <div className="status-row">
          <span className="status-label">Credential Vault</span>
          <span className="status-val">OS Keyring Bound</span>
        </div>
        <div className="status-row">
          <span className="status-label">Trust Verifier</span>
          <span className="status-val status-verified">Ed25519 / P-256 JWK</span>
        </div>
      </section>

      {currentTask && (
        <section className="outbox-telemetry">
          <div className="outbox-row">
            <span className="outbox-label">Outbox State:</span>
            <code className="outbox-state">{currentTask.state}</code>
          </div>
          <div className="outbox-row">
            <span className="outbox-label">CAS Revision:</span>
            <span className="outbox-val">#{currentTask.revision}</span>
          </div>
        </section>
      )}
    </aside>
  );
};
