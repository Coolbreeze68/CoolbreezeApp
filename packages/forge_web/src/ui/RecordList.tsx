import { Avatar, Card, Group, Paper, ScrollArea, Stack, Table, Text, UnstyledButton } from '@mantine/core';
import { useMediaQuery } from '@mantine/hooks';
import { IconChevronDown, IconChevronRight, IconChevronUp, IconSelector } from '@tabler/icons-react';
import { useState } from 'react';
import { useNavigate } from 'react-router';

import type { Json } from '../api/client';
import { type ListQuery, pageCount, queryKey, type Sort } from '../api/query';
import { useForge } from '../context';
import { useAsync } from '../hooks';
import { paths } from '../paths';
import { isNumeric, type TableSchema, visibleColumns } from '../schema';
import { Empty, ErrorView, Loading, Pager, ValueView } from './common';

/** Initiales d'un intitulé, pour l'avatar des tuiles. */
const initials = (title: string) =>
  title
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((w) => w[0]?.toUpperCase())
    .join('') || '#';

/**
 * Liste paginée des enregistrements de `table` correspondant à `query` (sa
 * page est ignorée) : tableau sur grand écran, tuiles sinon.
 */
export function RecordList({
  table,
  query = {},
  compact = false,
  onSort,
}: {
  table: TableSchema;
  query?: ListQuery;
  /** Tuiles même sur grand écran (listes liées d'une fiche). */
  compact?: boolean;
  onSort?: (sort: Sort[]) => void;
}) {
  const forge = useForge();
  const navigate = useNavigate();
  const wide = useMediaQuery('(min-width: 62em)', true);
  const [page, setPage] = useState(1);
  const key = queryKey(query);
  const [lastKey, setLastKey] = useState(key);
  if (key !== lastKey) {
    // Nouvelle requête : retour à la première page.
    setLastKey(key);
    setPage(1);
  }
  const listing = useAsync(
    () => forge.client.table(table.name).list({ ...query, page }),
    [table.name, key, page],
  );

  if (listing.error) return <ErrorView error={listing.error} onRetry={listing.reload} />;
  if (!listing.data) return <Loading />;
  const { items, total, perPage } = listing.data;
  if (!items.length) return <Empty />;
  const first = (listing.data.page - 1) * perPage + 1;
  const pager = (
    <Pager
      page={listing.data.page}
      pages={pageCount(listing.data)}
      first={first}
      last={first + items.length - 1}
      total={total}
      onPage={setPage}
    />
  );
  const open = (record: Json) => navigate(paths.record(table.name, record.id as number));

  if (compact || !wide) {
    const details = visibleColumns(table).filter(
      (c) => !c.title_field && !['reference', 'reference_list', 'text'].includes(c.type),
    );
    return (
      <Stack gap={0}>
        {items.map((record) => {
          const title = forge.format.title(table, record);
          const subtitle = details
            .map((c) => [c, forge.format.format(table, c, record[c.name])] as const)
            .filter(([, text]) => text)
            .slice(0, 2)
            .map(([c, text]) => `${forge.format.columnLabel(c)} : ${text}`)
            .join(' · ');
          return (
            <UnstyledButton
              key={record.id as number}
              onClick={() => open(record)}
              px="md"
              py="sm"
              style={{ borderBottom: '1px solid var(--mantine-color-default-border)' }}
            >
              <Group wrap="nowrap">
                <Avatar color="indigo" radius="xl" variant="light">
                  {initials(title)}
                </Avatar>
                <div style={{ flex: 1, minWidth: 0 }}>
                  <Text fw={600} truncate>
                    {title}
                  </Text>
                  {subtitle && (
                    <Text size="sm" c="dimmed" truncate>
                      {subtitle}
                    </Text>
                  )}
                </div>
                <IconChevronRight size={18} color="var(--mantine-color-dimmed)" />
              </Group>
            </UnstyledButton>
          );
        })}
        {pageCount(listing.data) > 1 || !compact ? pager : null}
      </Stack>
    );
  }

  const columns = visibleColumns(table);
  const sorted = query.sort?.[0];
  return (
    <Card p={0}>
      <ScrollArea>
        <Table highlightOnHover verticalSpacing="sm" horizontalSpacing="md" striped="even">
          <Table.Thead>
            <Table.Tr>
              {columns.map((c) => {
                const sortable = !c.virtual && onSort;
                const active = sorted?.column === c.name;
                const Icon = !active ? IconSelector : sorted?.descending ? IconChevronDown : IconChevronUp;
                return (
                  <Table.Th key={c.name} style={{ textAlign: isNumeric(c.type) ? 'right' : undefined }}>
                    {sortable ? (
                      <UnstyledButton
                        onClick={() => onSort([{ column: c.name, descending: active && !sorted?.descending }])}
                      >
                        <Group gap={4} wrap="nowrap" justify={isNumeric(c.type) ? 'flex-end' : undefined}>
                          <Text fw={700} size="sm">
                            {forge.format.columnLabel(c)}
                          </Text>
                          <Icon size={14} />
                        </Group>
                      </UnstyledButton>
                    ) : (
                      <Text fw={700} size="sm">
                        {forge.format.columnLabel(c)}
                      </Text>
                    )}
                  </Table.Th>
                );
              })}
            </Table.Tr>
          </Table.Thead>
          <Table.Tbody>
            {items.map((record) => (
              <Table.Tr key={record.id as number} onClick={() => open(record)} style={{ cursor: 'pointer' }}>
                {columns.map((c) => (
                  <Table.Td
                    key={c.name}
                    style={{
                      maxWidth: 280,
                      textAlign: isNumeric(c.type) ? 'right' : undefined,
                      fontVariantNumeric: 'tabular-nums',
                      // Textes sur plusieurs lignes ; le reste (nombres, dates, pastilles) sur une.
                      whiteSpace: c.type === 'string' || c.type === 'text' ? undefined : 'nowrap',
                    }}
                  >
                    <ValueView table={table} column={c} record={record} links={false} />
                  </Table.Td>
                ))}
              </Table.Tr>
            ))}
          </Table.Tbody>
        </Table>
      </ScrollArea>
      <Paper radius={0} style={{ borderTop: '1px solid var(--mantine-color-default-border)' }}>
        {pager}
      </Paper>
    </Card>
  );
}
