import {
  Button,
  Chip,
  Group,
  Modal,
  NumberInput,
  SegmentedControl,
  Stack,
  TextInput,
} from '@mantine/core';
import { DatePickerInput } from '@mantine/dates';
import { useState } from 'react';

import { equals, type Filter, oneOf } from '../api/query';
import { useForge } from '../context';
import type { ColumnSchema, TableSchema } from '../schema';
import { isDecimal, isFile, isText, visibleColumns } from '../schema';
import { dateTimeToJson, jsonToDate, parsePercentInput, percentText } from '../values';
import { ReferenceSelect } from './ReferenceSelect';

/** Filtre posé par l'utilisateur sur une colonne. */
export type ColumnFilter =
  | { kind: 'choice'; column: ColumnSchema; values: string[] }
  | { kind: 'bool'; column: ColumnSchema; value: boolean }
  | { kind: 'text'; column: ColumnSchema; text: string }
  | { kind: 'reference'; column: ColumnSchema; id: number }
  | { kind: 'equals'; column: ColumnSchema; value: string }
  /** Bornes incluses, au format de l'API ; dates en jours pour une date-heure. */
  | { kind: 'range'; column: ColumnSchema; min: string | null; max: string | null };

/** Colonnes filtrables : stockées et affichées. */
export const filterableColumns = (table: TableSchema) =>
  visibleColumns(table).filter((c) => !c.virtual && c.type !== 'reference_list' && !isFile(c.type));

/** Colonne filtrée par « contient ». */
const isTextual = (column: ColumnSchema) => isText(column.type) || column.type === 'color';

/** Filtres d'API d'un filtre de colonne. */
export function toApiFilters(filter: ColumnFilter): Filter[] {
  const name = filter.column.name;
  switch (filter.kind) {
    case 'choice':
      return [oneOf(name, filter.values)];
    case 'bool':
      return [equals(name, String(filter.value))];
    case 'text':
      return [{ column: name, op: 'like', value: filter.text }];
    case 'reference':
      return [equals(name, String(filter.id))];
    case 'equals':
      return [equals(name, filter.value)];
    case 'range': {
      if (filter.column.type === 'datetime') {
        // Journées entières : du début du premier jour au début du lendemain du dernier.
        const from = jsonToDate(filter.min);
        const to = jsonToDate(filter.max);
        return [
          ...(from ? [{ column: name, op: 'gte' as const, value: dateTimeToJson(from) }] : []),
          ...(to
            ? [
                {
                  column: name,
                  op: 'lt' as const,
                  value: dateTimeToJson(new Date(to.getFullYear(), to.getMonth(), to.getDate() + 1)),
                },
              ]
            : []),
        ];
      }
      return [
        ...(filter.min !== null ? [{ column: name, op: 'gte' as const, value: filter.min }] : []),
        ...(filter.max !== null ? [{ column: name, op: 'lte' as const, value: filter.max }] : []),
      ];
    }
  }
}

/** Filtre d'égalité lu dans l'URL (`?entreprise=3`), au format de l'API. */
export function filterFromParameter(column: ColumnSchema, value: string): ColumnFilter | null {
  switch (column.type) {
    case 'reference':
      return Number.isInteger(Number(value)) ? { kind: 'reference', column, id: Number(value) } : null;
    case 'enum':
      return { kind: 'choice', column, values: [value] };
    case 'boolean':
      return { kind: 'bool', column, value: value === 'true' };
    default:
      return { kind: 'equals', column, value };
  }
}

/** Texte d'un filtre actif, hors références (affichées par leur intitulé). */
export function useDescribeFilter() {
  const { format } = useForge();
  return (table: TableSchema, filter: ColumnFilter): string => {
    const column = filter.column;
    const value = (json: string) =>
      format.format(table, column.type === 'datetime' ? { ...column, type: 'date' } : column, json);
    switch (filter.kind) {
      case 'choice':
        return filter.values.map((v) => format.enumLabel(table, column, v)).join(', ');
      case 'bool':
        return filter.value ? format.strings.yes : format.strings.no;
      case 'text':
        return `« ${filter.text} »`;
      case 'equals':
        return filter.value;
      case 'reference':
        return `#${filter.id}`;
      case 'range':
        if (filter.min !== null && filter.max !== null) return `${value(filter.min)} – ${value(filter.max)}`;
        return filter.min !== null ? `≥ ${value(filter.min)}` : `≤ ${value(filter.max!)}`;
    }
  };
}

/**
 * Fenêtre d'édition du filtre d'une colonne. `onApply` reçoit le nouveau
 * filtre, ou `null` pour le retirer.
 */
export function FilterModal({
  table,
  column,
  current,
  onApply,
  onClose,
}: {
  table: TableSchema;
  column: ColumnSchema;
  current: ColumnFilter | undefined;
  onApply: (filter: ColumnFilter | null) => void;
  onClose: () => void;
}) {
  const { format, strings } = useForge();
  const [choices, setChoices] = useState<string[]>(current?.kind === 'choice' ? current.values : []);
  const [bool, setBool] = useState<string>(current?.kind === 'bool' ? String(current.value) : 'all');
  const [text, setText] = useState(current?.kind === 'text' ? current.text : '');
  const [reference, setReference] = useState<number | null>(
    current?.kind === 'reference' ? current.id : null,
  );
  const [min, setMin] = useState<string | null>(current?.kind === 'range' ? current.min : null);
  const [max, setMax] = useState<string | null>(current?.kind === 'range' ? current.max : null);
  const minutes = column.type === 'duration';
  const percent = column.type === 'percent';

  const build = (): ColumnFilter | null => {
    if (isTextual(column)) return text.trim() ? { kind: 'text', column, text: text.trim() } : null;
    switch (column.type) {
      case 'enum':
        return choices.length ? { kind: 'choice', column, values: choices } : null;
      case 'boolean':
        return bool === 'all' ? null : { kind: 'bool', column, value: bool === 'true' };
      case 'reference':
        return reference === null ? null : { kind: 'reference', column, id: reference };
      default:
        return min === null && max === null ? null : { kind: 'range', column, min, max };
    }
  };

  const numberBound = (label: string, value: string | null, set: (v: string | null) => void) => (
    <NumberInput
      label={label}
      decimalSeparator={format.locale.startsWith('fr') ? ',' : '.'}
      allowDecimal={isDecimal(column.type)}
      suffix={minutes ? ` ${strings.minutesUnit}` : percent ? ' %' : undefined}
      value={value === null ? '' : minutes ? Number(value) / 60 : percent ? percentText(value) : Number(value)}
      onChange={(v) => {
        if (v === '' || v === null) return set(null);
        if (minutes) return set(String(Math.round(Number(v) * 60)));
        set(percent ? parsePercentInput(String(v)) : String(v));
      }}
    />
  );

  let editor;
  switch (isTextual(column) ? 'text' : column.type) {
    case 'enum':
      editor = (
        <Chip.Group multiple value={choices} onChange={setChoices}>
          <Group gap="xs">
            {(column.values ?? []).map((v) => (
              <Chip key={v} value={v}>
                {format.enumLabel(table, column, v)}
              </Chip>
            ))}
          </Group>
        </Chip.Group>
      );
      break;
    case 'boolean':
      editor = (
        <SegmentedControl
          value={bool}
          onChange={setBool}
          data={[
            { value: 'all', label: strings.all },
            { value: 'true', label: strings.yes },
            { value: 'false', label: strings.no },
          ]}
        />
      );
      break;
    case 'text':
      editor = (
        <TextInput
          label={strings.contains}
          data-autofocus
          value={text}
          onChange={(e) => setText(e.currentTarget.value)}
          onKeyDown={(e) => e.key === 'Enter' && onApply(build())}
        />
      );
      break;
    case 'reference':
      editor = <ReferenceSelect target={column.target!} value={reference} onChange={setReference} />;
      break;
    case 'date':
    case 'datetime':
      editor = (
        <DatePickerInput
          type="range"
          allowSingleDateInRange
          clearable
          label={`${strings.from} – ${strings.to}`}
          value={[min, max]}
          onChange={([from, to]) => {
            // Valeurs `AAAA-MM-JJ` : pas de conversion en date (fuseau horaire).
            setMin(from ? String(from).slice(0, 10) : null);
            setMax(to ? String(to).slice(0, 10) : null);
          }}
        />
      );
      break;
    default:
      editor = (
        <Group grow>
          {numberBound(strings.min, min, setMin)}
          {numberBound(strings.max, max, setMax)}
        </Group>
      );
  }

  return (
    <Modal opened onClose={onClose} title={format.columnLabel(column)} centered>
      <Stack>
        {editor}
        <Group justify="flex-end">
          <Button variant="default" onClick={() => onApply(null)}>
            {strings.clear}
          </Button>
          <Button onClick={() => onApply(build())}>{strings.apply}</Button>
        </Group>
      </Stack>
    </Modal>
  );
}
