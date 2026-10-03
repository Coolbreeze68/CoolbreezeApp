import { Box, Card, Group, SimpleGrid, Stack, Text, ThemeIcon, Title, Tooltip, useComputedColorScheme } from '@mantine/core';
import { IconHash, IconSum } from '@tabler/icons-react';

import { type ListQuery, queryKey } from '../api/query';
import type { AggregateGroup, Measure } from '../api/table';
import { type Forge, useForge } from '../context';
import { useAsync } from '../hooks';
import { NEUTRAL, enumColor } from '../palette';
import { type ColumnSchema, findColumn, type TableSchema } from '../schema';
import { ErrorView, Loading, RecordLink } from './common';

/** Valeur d'une mesure, mise en forme selon la colonne. */
export function formatMeasure(forge: Forge, column: ColumnSchema, value: number | null): string {
  if (value === null) return '–';
  return column.type === 'duration'
    ? forge.format.duration(Math.round(value))
    : forge.format.number(Math.round(value * 100) / 100);
}

/** Libellé d'une valeur de regroupement. */
function GroupLabel({ table, column, value }: { table: TableSchema; column: ColumnSchema; value: unknown }) {
  const forge = useForge();
  if (value === null || value === undefined) return <>{forge.strings.none}</>;
  if (column.type === 'reference' && typeof value === 'number') {
    return <RecordLink table={column.target!} id={value} link={false} />;
  }
  return <>{forge.format.format(table, column, value)}</>;
}

/** Indicateur : icône en dégradé, valeur, détail. */
export function StatTile({
  label,
  value,
  details,
  icon,
  gradient = { from: 'indigo', to: 'violet' },
}: {
  label: string;
  value: string;
  details?: string;
  icon: React.ReactNode;
  gradient?: { from: string; to: string };
}) {
  return (
    <Card>
      <Group align="flex-start" wrap="nowrap">
        <ThemeIcon size={44} radius="md" variant="gradient" gradient={{ ...gradient, deg: 135 }}>
          {icon}
        </ThemeIcon>
        <div style={{ minWidth: 0 }}>
          <Text size="sm" c="dimmed" fw={600}>
            {label}
          </Text>
          <Text fz={26} fw={800} lh={1.2}>
            {value}
          </Text>
          {details && (
            <Text size="xs" c="dimmed">
              {details}
            </Text>
          )}
        </div>
      </Group>
    </Card>
  );
}

/**
 * Barres horizontales, une par groupe. La couleur suit la valeur de
 * regroupement (énumération) ; chaque barre porte son libellé et sa valeur.
 */
export function GroupBars({
  title,
  table,
  groupBy,
  groups,
  value,
  format,
  tooltip,
}: {
  title: string;
  table: TableSchema;
  groupBy: ColumnSchema;
  groups: AggregateGroup[];
  value: (group: AggregateGroup) => number;
  format: (value: number) => string;
  tooltip?: (group: AggregateGroup) => string;
}) {
  const scheme = useComputedColorScheme('light');
  const largest = Math.max(0, ...groups.map((g) => Math.abs(value(g))));
  // Énumération : ordre de déclaration des valeurs ; groupe sans valeur en dernier.
  const rank = (g: AggregateGroup) =>
    g.key === null ? Infinity : groupBy.type === 'enum' ? (groupBy.values ?? []).indexOf(String(g.key)) : 0;
  const ordered = [...groups].sort((a, b) => rank(a) - rank(b));
  const color = (g: AggregateGroup) =>
    groupBy.type === 'enum' && typeof g.key === 'string'
      ? enumColor(groupBy.values, g.key, scheme)
      : g.key === null
        ? NEUTRAL[scheme]
        : 'var(--mantine-primary-color-filled)';
  return (
    <Card>
      <Title order={5} mb="md">
        {title}
      </Title>
      <Stack gap={10}>
        {ordered.map((g, i) => (
          <Tooltip key={i} label={tooltip?.(g) ?? format(value(g))} position="top-start" withArrow>
            <Group wrap="nowrap" gap="sm">
              <Text size="sm" w={150} truncate>
                <GroupLabel table={table} column={groupBy} value={g.key} />
              </Text>
              <Box style={{ flex: 1 }} h={22}>
                <Box
                  h="100%"
                  w={`${largest ? (Math.abs(value(g)) / largest) * 100 : 0}%`}
                  miw={4}
                  style={{ background: color(g), borderRadius: '0 4px 4px 0' }}
                />
              </Box>
              <Text size="sm" fw={600} w={110} ta="right" style={{ fontVariantNumeric: 'tabular-nums' }}>
                {format(value(g))}
              </Text>
            </Group>
          </Tooltip>
        ))}
      </Stack>
    </Card>
  );
}

/** Statistiques de la vue `stats` : totaux et répartition par groupe. */
export function StatsPanel({ table, query }: { table: TableSchema; query: ListQuery }) {
  const forge = useForge();
  const { strings, format } = forge;
  const view = table.stats!;
  const aggregation = useAsync(
    () => forge.client.table(table.name).aggregate(view.fields, view.group_by, query),
    [table.name, queryKey(query)],
  );
  if (aggregation.error) return <ErrorView error={aggregation.error} onRetry={aggregation.reload} />;
  if (!aggregation.data) return <Loading />;
  const data = aggregation.data;
  const fields = data.fields.map((f) => findColumn(table, f)).filter((c): c is ColumnSchema => !!c);
  const groupBy = data.groupBy ? findColumn(table, data.groupBy) : undefined;
  const details = (column: ColumnSchema, m: Measure | undefined) =>
    `${strings.average} ${formatMeasure(forge, column, m?.avg ?? null)} · ${strings.min} ${formatMeasure(forge, column, m?.min ?? null)} · ${strings.max} ${formatMeasure(forge, column, m?.max ?? null)}`;

  return (
    <Stack gap="lg">
      <SimpleGrid cols={{ base: 1, sm: 2, lg: 3 }}>
        <StatTile label={strings.count} value={format.number(data.total.count)} icon={<IconHash size={22} />} />
        {fields.map((c) => (
          <StatTile
            key={c.name}
            label={`${format.columnLabel(c)} (${strings.sum})`}
            value={formatMeasure(forge, c, data.total.measures[c.name]?.sum ?? null)}
            details={details(c, data.total.measures[c.name])}
            icon={<IconSum size={22} />}
            gradient={{ from: 'teal', to: 'cyan' }}
          />
        ))}
      </SimpleGrid>
      {groupBy && data.groups.length > 0 && (
        <SimpleGrid cols={{ base: 1, lg: 2 }}>
          <GroupBars
            title={`${strings.count} · ${format.columnLabel(groupBy)}`}
            table={table}
            groupBy={groupBy}
            groups={data.groups}
            value={(g) => g.count}
            format={format.number}
          />
          {fields.map((c) => (
            <GroupBars
              key={c.name}
              title={`${format.columnLabel(c)} (${strings.sum}) · ${format.columnLabel(groupBy)}`}
              table={table}
              groupBy={groupBy}
              groups={data.groups}
              value={(g) => g.measures[c.name]?.sum ?? 0}
              format={(v) => formatMeasure(forge, c, v)}
              tooltip={(g) => `${strings.count} ${g.count} · ${details(c, g.measures[c.name])}`}
            />
          ))}
        </SimpleGrid>
      )}
    </Stack>
  );
}
