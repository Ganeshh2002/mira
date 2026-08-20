import { Row } from '../components/Row';
import { Section } from '../components/Section';
import { commands } from '../lib/ipc';
import { Button } from '../components/Button';
import type { FoundationStatus } from '../bindings/FoundationStatus';

/**
 * The shell verification screen.
 *
 * Slice 0 deliberately stops here. The project list, the detail pane, and every
 * domain section belong to Slice 1 and later; showing a hollow version of them now
 * would be the "looks finished, does nothing" state the roadmap is shaped to avoid.
 */
export function Foundation({ status }: { status: FoundationStatus }) {
  return (
    <Section label="Foundation">
      <Row label="Platform" value={status.platform} />
      <Row label="Session" value={status.session} />
      <Row label="Schema" value={`v${status.schemaVersion}`} />
      <Row
        label="Shortcut"
        value={status.shortcutChord}
        detail={
          status.shortcutRegistered
            ? undefined
            : 'Not registered on this machine. Settings explains why.'
        }
      />
      <Row label="Status" value="Ready" />
      <div className="flex justify-end p-[var(--space-3)]">
        <Button onClick={() => void commands.openSettings()}>Settings</Button>
      </div>
    </Section>
  );
}
