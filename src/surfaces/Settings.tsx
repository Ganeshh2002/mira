import { Chip } from '../components/Chip';
import { Row } from '../components/Row';
import { Section } from '../components/Section';
import { StatusMark, statusName } from '../components/StatusMark';
import type { CapabilityStatus } from '../bindings/CapabilityStatus';
import type { FoundationStatus } from '../bindings/FoundationStatus';

/**
 * Settings is app-scoped and lives in its own window (information-architecture
 * §4.5). In Slice 0 it holds what the foundation can honestly report: where the
 * data lives, and what this machine can and cannot do.
 *
 * A capability that is off renders as off **with its reason**. It is never hidden
 * silently and never shown as broken (platform-abstraction §2 rule 4).
 */
function explain(status: CapabilityStatus): string | undefined {
  switch (status.state) {
    case 'full':
      return undefined;
    case 'degraded':
      return `${status.reason}. ${status.detail}`;
    case 'unavailable':
      return status.fallback
        ? `${status.reason}. Use ${status.fallback} instead.`
        : `${status.reason}.`;
  }
}

export function Settings({ status }: { status: FoundationStatus }) {
  return (
    <div className="flex flex-col gap-[var(--section-gap)]">
      <Section label="About">
        <Row label="Version" value={status.appVersion} />
        <Row label="Platform" value={`${status.platform} · ${status.session}`} />
      </Section>

      <Section label="Privacy">
        <Row
          label="Database"
          value={status.databasePath}
          detail="Everything Mira knows is in this one file. Deleting it returns Mira to first run."
        />
        <Row label="Projects" value={String(status.projectCount)} />
        <Row label="Workspaces" value={String(status.workspaceCount)} />
      </Section>

      <Section label="What this machine can do">
        {status.capabilities.map((report) => (
          <Row
            key={report.capability}
            mark={<StatusMark status={report.status} />}
            label={report.label}
            value={<Chip>{statusName(report.status)}</Chip>}
            detail={explain(report.status)}
          />
        ))}
      </Section>
    </div>
  );
}
