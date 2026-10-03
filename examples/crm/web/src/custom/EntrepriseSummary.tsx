import { type DetailSectionProps, equals, useAsync, useForge } from '@forge/web';
import { Card, Group, Text, ThemeIcon } from '@mantine/core';
import { IconTrophy } from '@tabler/icons-react';

import { opportuniteApi } from '../generated/models';

/**
 * Bloc de la fiche entreprise : opportunités gagnées et leur montant, lues avec
 * le client typé `opportuniteApi`.
 */
export function EntrepriseSummary({ record }: DetailSectionProps) {
  const forge = useForge();
  const won = useAsync(
    () =>
      opportuniteApi(forge.client).listAll({
        filters: [equals('entreprise', String(record.id)), equals('etape', 'gagne')],
      }),
    [record.id],
  );
  if (!won.data) return null;
  const total = won.data.reduce((sum, o) => sum + Number(o.montant), 0);
  const french = forge.locale === 'fr';
  return (
    <Card>
      <Group>
        <ThemeIcon size={44} radius="md" variant="gradient" gradient={{ from: 'yellow', to: 'orange', deg: 135 }}>
          <IconTrophy size={24} />
        </ThemeIcon>
        <div>
          <Text fw={700}>
            {french ? `${won.data.length} opportunité(s) gagnée(s)` : `${won.data.length} won opportunity(ies)`}
          </Text>
          <Text size="sm" c="dimmed">
            {french ? 'Montant HT' : 'Amount'} : {forge.format.number(total)}
          </Text>
        </div>
      </Group>
    </Card>
  );
}
