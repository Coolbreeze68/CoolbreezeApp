import { ActionIcon, Badge, Button, Card, Group, SimpleGrid, Stack, Text, UnstyledButton } from '@mantine/core';
import { useMediaQuery } from '@mantine/hooks';
import { IconChevronLeft, IconChevronRight, IconPlus } from '@tabler/icons-react';
import { useState } from 'react';
import { Link, useNavigate } from 'react-router';

import type { ForgeClient, Json } from '../api/client';
import { type ListQuery, queryKey, withFilters } from '../api/query';
import { can, useForge } from '../context';
import { useAsync } from '../hooks';
import { paths } from '../paths';
import { findColumn, type TableSchema } from '../schema';
import { dateTimeToJson, dateToJson, jsonToDate, jsonToDateTime } from '../values';
import { ErrorView } from './common';

/** Enregistrement placé dans le calendrier. */
export interface CalendarEvent {
  record: Json;
  start: Date;
  /** Fin, si la vue en a une (colonne de fin ou durée). */
  end: Date | null;
  /** Colonne de type date : pas d'heure. */
  allDay: boolean;
}

const day = (t: Date) => new Date(t.getFullYear(), t.getMonth(), t.getDate());
const sameDay = (a: Date, b: Date) => day(a).getTime() === day(b).getTime();

export const occursOn = (event: CalendarEvent, d: Date) =>
  day(event.start) <= day(d) && day(d) <= day(event.end ?? event.start);

/** Événements de `table` dont une partie tombe dans [from, to[. */
export async function loadEvents(
  client: ForgeClient,
  table: TableSchema,
  query: ListQuery,
  from: Date,
  to: Date,
): Promise<CalendarEvent[]> {
  const view = table.calendar!;
  const allDay = findColumn(table, view.start)?.type === 'date';
  const bound = (d: Date) => (allDay ? dateToJson(d) : dateTimeToJson(d));
  const records = client.table(table.name);
  const base: ListQuery = { ...query, sort: [{ column: view.start }] };
  const found = new Map<number, Json>();
  const inRange = await records.listAll(
    withFilters(base, [
      { column: view.start, op: 'gte', value: bound(from) },
      { column: view.start, op: 'lt', value: bound(to) },
    ]),
  );
  for (const r of inRange) found.set(r.id as number, r);
  // Commencés avant la période et pas encore finis.
  if (view.end) {
    const ongoing = await records.listAll(
      withFilters(base, [
        { column: view.start, op: 'lt', value: bound(from) },
        { column: view.end, op: 'gte', value: bound(from) },
      ]),
    );
    for (const r of ongoing) found.set(r.id as number, r);
  }
  const read = (json: unknown) => (allDay ? jsonToDate(json) : jsonToDateTime(json));
  const events: CalendarEvent[] = [];
  for (const record of found.values()) {
    const start = read(record[view.start]);
    if (!start) continue;
    let end: Date | null = null;
    if (view.end) end = read(record[view.end]);
    else if (view.duration && typeof record[view.duration] === 'number') {
      end = new Date(start.getTime() + (record[view.duration] as number) * 1000);
    }
    events.push({ record, start, end, allDay });
  }
  return events.sort((a, b) => a.start.getTime() - b.start.getTime());
}

/** Vue mensuelle et agenda du jour choisi. */
export function CalendarPanel({ table, query }: { table: TableSchema; query: ListQuery }) {
  const forge = useForge();
  const { format, strings } = forge;
  const navigate = useNavigate();
  const wide = useMediaQuery('(min-width: 62em)', true);
  const today = day(new Date());
  const [selected, setSelected] = useState(today);
  const [month, setMonth] = useState(new Date(today.getFullYear(), today.getMonth(), 1));
  // Premier jour affiché : le lundi de la semaine du 1er du mois.
  const gridStart = new Date(month.getFullYear(), month.getMonth(), 1 - ((month.getDay() + 6) % 7));
  const gridEnd = new Date(gridStart.getFullYear(), gridStart.getMonth(), gridStart.getDate() + 42);
  const events = useAsync(
    () => loadEvents(forge.client, table, query, gridStart, gridEnd),
    [table.name, queryKey(query), gridStart.getTime()],
  );
  if (events.error) return <ErrorView error={events.error} onRetry={events.reload} />;
  const list = events.data ?? [];
  const showMonth = (m: Date, select?: Date) => {
    setMonth(new Date(m.getFullYear(), m.getMonth(), 1));
    setSelected(select ?? new Date(m.getFullYear(), m.getMonth(), 1));
  };
  const weekday = new Intl.DateTimeFormat(forge.locale.replace('_', '-'), { weekday: 'short' });
  const monthTitle = new Intl.DateTimeFormat(forge.locale.replace('_', '-'), {
    month: 'long',
    year: 'numeric',
  }).format(month);
  const dayTitle = new Intl.DateTimeFormat(forge.locale.replace('_', '-'), {
    weekday: 'long',
    day: 'numeric',
    month: 'long',
  }).format(selected);
  const startColumn = findColumn(table, table.calendar!.start)!;
  const startValue =
    startColumn.type === 'date'
      ? dateToJson(selected)
      : dateTimeToJson(new Date(selected.getFullYear(), selected.getMonth(), selected.getDate(), 9));
  const dayEvents = list.filter((e) => occursOn(e, selected));
  const time = (e: CalendarEvent) =>
    e.allDay ? strings.allDay : `${format.time(e.start)}${e.end ? ` – ${format.time(e.end)}` : ''}`;

  const grid = (
    <Card p="sm">
      <Group justify="space-between" mb="sm">
        <Group gap={4}>
          <ActionIcon variant="subtle" aria-label={strings.previousMonth} onClick={() => showMonth(new Date(month.getFullYear(), month.getMonth() - 1))}>
            <IconChevronLeft size={18} />
          </ActionIcon>
          <ActionIcon variant="subtle" aria-label={strings.nextMonth} onClick={() => showMonth(new Date(month.getFullYear(), month.getMonth() + 1))}>
            <IconChevronRight size={18} />
          </ActionIcon>
          <Text fw={700} tt="capitalize">
            {monthTitle}
          </Text>
        </Group>
        <Button size="xs" variant="light" onClick={() => showMonth(today, today)}>
          {strings.today}
        </Button>
      </Group>
      <SimpleGrid cols={7} spacing={4} verticalSpacing={4}>
        {Array.from({ length: 7 }, (_, i) => (
          <Text key={i} size="xs" c="dimmed" ta="center" tt="capitalize">
            {weekday.format(new Date(gridStart.getFullYear(), gridStart.getMonth(), gridStart.getDate() + i))}
          </Text>
        ))}
        {Array.from({ length: 42 }, (_, i) => {
          const d = new Date(gridStart.getFullYear(), gridStart.getMonth(), gridStart.getDate() + i);
          const items = list.filter((e) => occursOn(e, d));
          const isSelected = sameDay(d, selected);
          const isToday = sameDay(d, today);
          return (
            <UnstyledButton
              key={i}
              onClick={() => setSelected(d)}
              p={4}
              h={wide ? 96 : 44}
              style={{
                borderRadius: 'var(--mantine-radius-md)',
                overflow: 'hidden',
                background: isSelected ? 'var(--mantine-color-indigo-light)' : undefined,
                border: '1px solid var(--mantine-color-default-border)',
                opacity: d.getMonth() === month.getMonth() ? 1 : 0.45,
              }}
            >
              <Text size="sm" fw={isToday ? 800 : 500} c={isToday ? 'indigo' : undefined}>
                {d.getDate()}
              </Text>
              {wide ? (
                <Stack gap={2}>
                  {items.slice(0, 2).map((e) => (
                    <Badge key={e.record.id as number} size="xs" variant="light" fullWidth styles={{ label: { textTransform: 'none' } }}>
                      {format.title(table, e.record)}
                    </Badge>
                  ))}
                  {items.length > 2 && (
                    <Text size="xs" c="dimmed">
                      +{items.length - 2}
                    </Text>
                  )}
                </Stack>
              ) : (
                items.length > 0 && (
                  <div
                    style={{
                      width: 6,
                      height: 6,
                      borderRadius: 3,
                      margin: '2px auto 0',
                      background: 'var(--mantine-color-indigo-6)',
                    }}
                  />
                )
              )}
            </UnstyledButton>
          );
        })}
      </SimpleGrid>
    </Card>
  );

  const agenda = (
    <Card>
      <Group justify="space-between" mb="sm">
        <Text fw={700} tt="capitalize">
          {dayTitle}
        </Text>
        {can(forge, table, 'create') && (
          <ActionIcon
            variant="light"
            component={Link}
            to={paths.create(table.name, { [startColumn.name]: startValue })}
            aria-label={strings.create}
          >
            <IconPlus size={18} />
          </ActionIcon>
        )}
      </Group>
      {dayEvents.length === 0 ? (
        <Text c="dimmed" size="sm">
          {strings.noEvents}
        </Text>
      ) : (
        <Stack gap="xs">
          {dayEvents.map((e) => (
            <UnstyledButton
              key={e.record.id as number}
              onClick={() => navigate(paths.record(table.name, e.record.id as number))}
              p="xs"
              style={{ borderRadius: 'var(--mantine-radius-md)', borderLeft: '4px solid var(--mantine-color-indigo-6)' }}
            >
              <Text size="xs" c="dimmed">
                {time(e)}
              </Text>
              <Text fw={600}>{format.title(table, e.record)}</Text>
            </UnstyledButton>
          ))}
        </Stack>
      )}
    </Card>
  );

  return wide ? (
    <Group align="flex-start" wrap="nowrap" gap="md">
      <div style={{ flex: 1 }}>{grid}</div>
      <div style={{ width: 320 }}>{agenda}</div>
    </Group>
  ) : (
    <Stack>
      {grid}
      {agenda}
    </Stack>
  );
}
