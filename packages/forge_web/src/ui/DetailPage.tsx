import { ActionIcon, Anchor, Button, Card, Group, SimpleGrid, Stack, Text, Title, Tooltip } from '@mantine/core';
import { modals } from '@mantine/modals';
import { IconArrowLeft, IconPencil, IconPlus, IconTrash } from '@tabler/icons-react';
import { Link, useNavigate } from 'react-router';

import { equals } from '../api/query';
import { can, useForge } from '../context';
import { useAsync } from '../hooks';
import { paths } from '../paths';
import { relatedLists, type TableSchema, visibleColumns } from '../schema';
import { jsonToDateTime } from '../values';
import { ErrorView, Loading, useNotify, ValueView } from './common';
import { RecordList } from './RecordList';

/** Fiche d'un enregistrement : valeurs, blocs personnalisés, enregistrements qui le référencent. */
export function DetailPage({ table, id }: { table: TableSchema; id: number }) {
  const forge = useForge();
  const { strings, format, customization } = forge;
  const navigate = useNavigate();
  const notify = useNotify();
  const record = useAsync(() => forge.client.table(table.name).read(id), [table.name, id]);

  if (record.error) return <ErrorView error={record.error} onRetry={record.reload} />;
  if (!record.data) return <Loading />;
  const data = record.data;
  const title = format.title(table, data);
  const sections = customization.detailSections?.[table.name] ?? [];
  const related = relatedLists(forge.schema, table).filter((r) => can(forge, r.table, 'read'));
  const created = jsonToDateTime(data.created_at);
  const updated = jsonToDateTime(data.updated_at);

  const remove = () =>
    modals.openConfirmModal({
      title: strings.delete,
      children: <Text size="sm">{strings.confirmDelete(title)}</Text>,
      labels: { confirm: strings.delete, cancel: strings.cancel },
      confirmProps: { color: 'red' },
      onConfirm: async () => {
        try {
          await forge.client.table(table.name).delete(id);
          notify.success(strings.deleted);
          navigate(paths.table(table.name));
          forge.notifyChange();
        } catch (error) {
          notify.error(error);
        }
      },
    });

  return (
    <Stack gap="lg" maw={1100} mx="auto">
      <Group justify="space-between" wrap="nowrap">
        <Group gap="sm" wrap="nowrap" style={{ minWidth: 0 }}>
          <ActionIcon variant="subtle" size="lg" onClick={() => navigate(-1)} aria-label="←">
            <IconArrowLeft size={20} />
          </ActionIcon>
          <div style={{ minWidth: 0 }}>
            <Anchor component={Link} to={paths.table(table.name)} size="sm" c="dimmed">
              {format.tableLabel(table)}
            </Anchor>
            <Title order={2} lineClamp={1}>
              {title}
            </Title>
          </div>
        </Group>
        <Group gap="xs" wrap="nowrap">
          {can(forge, table, 'update') && (
            <Button component={Link} to={paths.edit(table.name, id)} leftSection={<IconPencil size={16} />}>
              {strings.edit}
            </Button>
          )}
          {can(forge, table, 'delete') && (
            <Tooltip label={strings.delete}>
              <ActionIcon variant="light" color="red" size="lg" onClick={remove} aria-label={strings.delete}>
                <IconTrash size={18} />
              </ActionIcon>
            </Tooltip>
          )}
        </Group>
      </Group>

      <Card>
        <SimpleGrid cols={{ base: 1, md: 2 }} spacing="lg" verticalSpacing="md">
          {visibleColumns(table).map((c) => (
            <div key={c.name} style={c.type === 'text' ? { gridColumn: '1 / -1' } : undefined}>
              <Text size="xs" fw={700} c="dimmed" tt="uppercase" mb={2}>
                {format.columnLabel(c)}
              </Text>
              <ValueView table={table} column={c} record={data} />
            </div>
          ))}
        </SimpleGrid>
        {created && updated && (
          <Text size="xs" c="dimmed" mt="lg">
            {strings.timestamps(format.dateTime(created), format.dateTime(updated))}
          </Text>
        )}
      </Card>

      {sections.map((Section, i) => (
        <Section key={i} table={table} record={data} />
      ))}

      {related.map(({ table: other, column }) => {
        const link = { [column.name]: String(id) };
        // Libellé de colonne ajouté si la table référence deux fois.
        const twice = related.filter((r) => r.table === other).length > 1;
        return (
          <Card key={`${other.name}.${column.name}`} p={0}>
            <Group justify="space-between" p="md" pb="xs">
              <Title order={4}>
                {format.tableLabel(other)}
                {twice && ` (${format.columnLabel(column)})`}
              </Title>
              <Group gap="xs">
                <Button variant="subtle" size="xs" component={Link} to={paths.table(other.name, link)}>
                  {strings.seeAll}
                </Button>
                {can(forge, other, 'create') && (
                  <ActionIcon variant="light" component={Link} to={paths.create(other.name, link)} aria-label={strings.create}>
                    <IconPlus size={16} />
                  </ActionIcon>
                )}
              </Group>
            </Group>
            <RecordList table={other} compact query={{ perPage: 5, filters: [equals(column.name, String(id))] }} />
          </Card>
        );
      })}
    </Stack>
  );
}
