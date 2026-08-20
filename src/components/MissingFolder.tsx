import { Row } from './Row';
import { Section } from './Section';

/**
 * The folder moved or was deleted.
 *
 * `prd.md` FR-1.5: a missing project is shown as missing and is never
 * auto-deleted. Mira states the fact and leaves the decision to the person.
 */
export function MissingFolder({ path }: { path: string }) {
  return (
    <Section label="Folder">
      <Row
        mark={<span className="text-ink-3 opacity-60">⌀</span>}
        label="Folder is missing"
        detail={`Nothing is at ${path} any more. It may have been moved or deleted. Mira has kept the project so nothing is lost.`}
      />
    </Section>
  );
}
