import { ActionIcon, Badge, Button, CloseButton, FileButton, Group, Menu, SegmentedControl, Stack, TextInput, Tooltip } from '@mantine/core';
import { useDebouncedValue } from '@mantine/hooks';
import { IconCalendarMonth, IconChartBar, IconDots, IconDownload, IconFilter, IconList, IconPlus, IconSearch, IconUpload } from '@tabler/icons-react';
import { useMemo, useState } from 'react';
import { Link, useSearchParams } from 'react-router';

import type { ListQuery, Sort } from '../api/query';
import { can, useForge } from '../context';
import { useRecordTitle } from '../hooks';
import { paths } from '../paths';
import { type ColumnSchema, isSearchable, type TableSchema } from '../schema';
import { CalendarPanel } from './CalendarPanel';
import { PageHeader, useNotify } from './common';
import { exportCsv, importCsv } from './csv';
import { type ColumnFilter, filterableColumns, FilterModal, filterFromParameter, toApiFilters, useDescribeFilter } from './filters';
import { RecordList } from './RecordList';
import { StatsPanel } from './StatsPanel';

type View = 'list' | 'calendar' | 'stats';

/** Puce d'un filtre actif ; une référence s'affiche par son intitulé. */
function FilterBadge({ table, filter, onEdit, onRemove }: { table: TableSchema; filter: ColumnFilter; onEdit: () => void; onRemove: () => void }) {
  const { format } = useForge();
  const describe = useDescribeFilter();
  const reference = useRecordTitle(filter.column.target ?? '', filter.kind === 'reference' ? filter.id : 0);
  const text = filter.kind === 'reference' ? reference : describe(table, filter);
  return (
    <Badge
      size="lg"
      variant="light"
      styles={{ label: { textTransform: 'none', cursor: 'pointer' } }}
      onClick={onEdit}
      rightSection={<CloseButton size="xs" onClick={(e) => { e.stopPropagation(); onRemove(); }} aria-label="×" />}
    >
      {format.columnLabel(filter.column)} : {text}
    </Badge>
  );
}

/** Page d'une table : liste, calendrier et statistiques, qui partagent recherche et filtres. */
export function TablePage({ table }: { table: TableSchema }) {
  const forge = useForge();
  const { strings, format } = forge;
  const notify = useNotify();
  const [params] = useSearchParams();
  const [view, setView] = useState<View>('list');
  const [search, setSearch] = useState('');
  const [debouncedSearch] = useDebouncedValue(search, 300);
  const [sort, setSort] = useState<Sort[]>([]);
  const [filters, setFilters] = useState<Record<string, ColumnFilter>>(() => {
    const initial: Record<string, ColumnFilter> = {};
    for (const column of filterableColumns(table)) {
      const value = params.get(column.name);
      const filter = value === null ? null : filterFromParameter(column, value);
      if (filter) initial[column.name] = filter;
    }
    return initial;
  });
  const [editing, setEditing] = useState<ColumnSchema | null>(null);

  const query: ListQuery = useMemo(
    () => ({ sort, search: debouncedSearch, filters: Object.values(filters).flatMap(toApiFilters) }),
    [sort, debouncedSearch, filters],
  );
  const views: { value: View; label: string; icon: React.ReactNode }[] = [
    { value: 'list', label: strings.listView, icon: <IconList size={16} /> },
    ...(table.calendar ? [{ value: 'calendar' as const, label: strings.calendarView, icon: <IconCalendarMonth size={16} /> }] : []),
    ...(table.stats ? [{ value: 'stats' as const, label: strings.statsView, icon: <IconChartBar size={16} /> }] : []),
  ];
  const canCreate = can(forge, table, 'create');
  const canImport = canCreate || can(forge, table, 'update');

  return (
    <Stack gap="md">
      <PageHeader
        title={format.tableLabel(table)}
        actions={
          <>
            {canCreate && (
              <Button component={Link} to={paths.create(table.name)} leftSection={<IconPlus size={18} />} variant="gradient">
                {strings.create}
              </Button>
            )}
            <Menu position="bottom-end">
              <Menu.Target>
                <ActionIcon variant="default" size="lg" aria-label={strings.more}>
                  <IconDots size={18} />
                </ActionIcon>
              </Menu.Target>
              <Menu.Dropdown>
                <Menu.Item leftSection={<IconDownload size={16} />} onClick={() => void exportCsv(forge, table, query, notify)}>
                  {strings.exportCsv}
                </Menu.Item>
                {canImport && (
                  <FileButton accept=".csv,text/csv" onChange={(file) => file && void importCsv(forge, table, file, notify)}>
                    {(props) => (
                      <Menu.Item {...props} leftSection={<IconUpload size={16} />} closeMenuOnClick={false}>
                        {strings.importCsv}
                      </Menu.Item>
                    )}
                  </FileButton>
                )}
              </Menu.Dropdown>
            </Menu>
          </>
        }
      />
      <Group gap="sm" wrap="wrap">
        {isSearchable(table) && (
          <TextInput
            style={{ flex: 1, minWidth: 220 }}
            placeholder={strings.search}
            aria-label={strings.search}
            leftSection={<IconSearch size={16} />}
            value={search}
            onChange={(e) => setSearch(e.currentTarget.value)}
          />
        )}
        <Menu position="bottom-end" width={240}>
          <Menu.Target>
            <Tooltip label={strings.filter}>
              <ActionIcon variant="light" size="lg" aria-label={strings.filter}>
                <IconFilter size={18} />
              </ActionIcon>
            </Tooltip>
          </Menu.Target>
          <Menu.Dropdown>
            {filterableColumns(table).map((c) => (
              <Menu.Item key={c.name} onClick={() => setEditing(c)}>
                {format.columnLabel(c)}
              </Menu.Item>
            ))}
          </Menu.Dropdown>
        </Menu>
        {views.length > 1 && (
          <SegmentedControl
            value={view}
            onChange={(v) => setView(v as View)}
            data={views.map((v) => ({
              value: v.value,
              label: (
                <Group gap={6} wrap="nowrap">
                  {v.icon}
                  <span>{v.label}</span>
                </Group>
              ),
            }))}
          />
        )}
      </Group>
      {Object.keys(filters).length > 0 && (
        <Group gap="xs">
          {Object.values(filters).map((f) => (
            <FilterBadge
              key={f.column.name}
              table={table}
              filter={f}
              onEdit={() => setEditing(f.column)}
              onRemove={() => setFilters(({ [f.column.name]: _, ...rest }) => rest)}
            />
          ))}
        </Group>
      )}
      {view === 'list' && <RecordList table={table} query={query} onSort={setSort} />}
      {view === 'calendar' && <CalendarPanel table={table} query={query} />}
      {view === 'stats' && <StatsPanel table={table} query={query} />}
      {editing && (
        <FilterModal
          table={table}
          column={editing}
          current={filters[editing.name]}
          onClose={() => setEditing(null)}
          onApply={(filter) => {
            setFilters(({ [editing.name]: _, ...rest }) => (filter ? { ...rest, [editing.name]: filter } : rest));
            setEditing(null);
          }}
        />
      )}
    </Stack>
  );
}
