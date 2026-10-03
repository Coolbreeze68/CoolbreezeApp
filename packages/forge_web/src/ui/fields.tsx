import { Group, NumberInput, Select, SimpleGrid, Switch, Textarea, TextInput } from '@mantine/core';
import { DateInput, DateTimePicker } from '@mantine/dates';
import { IconCalendar, IconCalendarClock, IconLink, IconMail, IconPhone } from '@tabler/icons-react';

import type { Json } from '../api/client';

import type { FieldProps } from '../customization';
import { useForge } from '../context';
import type { ColumnSchema, TableSchema } from '../schema';
import {
  datePresets,
  dateToJson,
  dateTimeToJson,
  isValidInput,
  jsonToDateTime,
  jsonToIds,
  localDateTimeText,
  parseDecimalInput,
  parseIntegerInput,
  parsePercentInput,
  percentText,
} from '../values';
import { CreateRecordButton } from './CreateRecord';
import { ColorField, FileField, MarkdownField, RatingField } from './models';
import { ReferenceMultiSelect, ReferenceSelect } from './ReferenceSelect';

/** Saisie qui ne correspond pas au type de la colonne (nombre mal écrit…). */
export class InvalidInput {
  constructor(readonly text: string) {}
}

/** Valeur absente : `null`, texte vide ou liste vide. */
export const isEmptyValue = (value: unknown) =>
  value === null || value === undefined || value === '' || (Array.isArray(value) && value.length === 0);

/** Erreur d'un champ requis vide ou mal saisi. */
export function clientError(strings: { invalidValue: string; required: string }, column: ColumnSchema, value: unknown) {
  if (value instanceof InvalidInput) return strings.invalidValue;
  if (column.required && isEmptyValue(value)) return strings.required;
  return undefined;
}

/** Valeur d'URL (format de l'API, en texte) convertie au JSON de la colonne. */
export function parseInitialValue(column: ColumnSchema, text: string): unknown {
  switch (column.type) {
    case 'integer':
    case 'duration':
    case 'rating':
    case 'reference':
      return Number.isInteger(Number(text)) ? Number(text) : null;
    case 'boolean':
      return text === 'true';
    case 'reference_list':
      return text.split(',').map(Number).filter(Number.isInteger);
    default:
      return text;
  }
}

const durationText = (seconds: number) =>
  `${Math.floor(seconds / 3600)}:${String(Math.floor((seconds % 3600) / 60)).padStart(2, '0')}`;

/** `h:mm`, ou un nombre de minutes. */
function parseDuration(text: string): number | null {
  const match = /^(\d+)(?::(\d{1,2}))?$/.exec(text.trim());
  if (!match) return null;
  const first = Number(match[1]);
  return match[2] === undefined ? first * 60 : first * 3600 + Number(match[2]) * 60;
}

/** Champ d'une colonne : personnalisé (`ForgeCustomization.fields`) ou par défaut. */
export function Field(props: FieldProps) {
  const { format, strings, customization, locale } = useForge();
  const { table, column, value, onChange, error } = props;
  const Custom = customization.fields?.[`${table.name}.${column.name}`];
  if (Custom) return <Custom {...props} />;
  const common = {
    label: format.columnLabel(column),
    withAsterisk: column.required,
    error,
  };
  const text = value instanceof InvalidInput ? value.text : value === null || value === undefined ? '' : String(value);

  switch (column.type) {
    case 'boolean':
      return (
        <Switch
          label={common.label}
          error={error}
          checked={value === true}
          onChange={(e) => onChange(e.currentTarget.checked)}
          mt="md"
        />
      );
    case 'enum':
      return (
        <Select
          {...common}
          data={(column.values ?? []).map((v) => ({ value: v, label: format.enumLabel(table, column, v) }))}
          value={typeof value === 'string' ? value : null}
          onChange={onChange}
          clearable={!column.required}
          allowDeselect={!column.required}
        />
      );
    case 'date':
      return (
        <DateInput
          {...common}
          locale={locale}
          valueFormat={locale.startsWith('fr') ? 'DD/MM/YYYY' : 'MM/DD/YYYY'}
          value={typeof value === 'string' ? value : null}
          onChange={(v) => onChange(v ? String(v).slice(0, 10) : null)}
          leftSection={<IconCalendar size={16} />}
          presets={datePresets(false).map((p) => ({ value: dateToJson(p.date), label: strings[p.label] }))}
          clearable
        />
      );
    case 'datetime':
      return (
        <DateTimePicker
          {...common}
          locale={locale}
          valueFormat={locale.startsWith('fr') ? 'DD/MM/YYYY HH:mm' : 'MM/DD/YYYY hh:mm A'}
          value={jsonToDateTime(value)}
          onChange={(v) => onChange(v ? dateTimeToJson(new Date(String(v).replace(' ', 'T'))) : null)}
          leftSection={<IconCalendarClock size={16} />}
          presets={datePresets(true).map((p) => ({ value: localDateTimeText(p.date), label: strings[p.label] }))}
          clearable
        />
      );
    case 'reference':
      return (
        <Group gap="xs" align="flex-end" wrap="nowrap">
          <ReferenceSelect
            {...common}
            target={column.target!}
            value={typeof value === 'number' ? value : null}
            onChange={onChange}
            required={column.required}
            style={{ flex: 1 }}
          />
          <CreateRecordButton target={column.target!} onCreated={onChange} />
        </Group>
      );
    case 'reference_list':
      return (
        <Group gap="xs" align="flex-end" wrap="nowrap">
          <ReferenceMultiSelect {...common} target={column.target!} value={jsonToIds(value)} onChange={onChange} style={{ flex: 1 }} />
          <CreateRecordButton target={column.target!} onCreated={(id) => onChange([...jsonToIds(value), id])} />
        </Group>
      );
    case 'color':
      return <ColorField {...props} label={common.label} />;
    case 'rating':
      return <RatingField {...props} label={common.label} />;
    case 'markdown':
      return <MarkdownField {...props} label={common.label} />;
    case 'file':
    case 'image':
      return <FileField {...props} label={common.label} />;
    case 'percent':
    case 'money':
      return (
        <TextInput
          {...common}
          inputMode="decimal"
          rightSection={column.type === 'percent' ? '%' : format.currencySymbol(column.currency ?? 'EUR')}
          defaultValue={(() => {
            const shown = column.type === 'percent' && typeof value === 'string' ? percentText(value) : text;
            return locale.startsWith('fr') ? shown.replace('.', ',') : shown;
          })()}
          onChange={(e) => {
            const input = e.currentTarget.value;
            const parsed = column.type === 'percent' ? parsePercentInput(input) : parseDecimalInput(input);
            onChange(input.trim() === '' ? null : (parsed ?? new InvalidInput(input)));
          }}
        />
      );
    case 'email':
    case 'url':
    case 'phone': {
      const Icon = column.type === 'email' ? IconMail : column.type === 'phone' ? IconPhone : IconLink;
      return (
        <TextInput
          {...common}
          type={column.type === 'phone' ? 'tel' : column.type}
          leftSection={<Icon size={16} />}
          maxLength={255}
          defaultValue={text}
          onChange={(e) => {
            const input = e.currentTarget.value.trim();
            onChange(input === '' ? null : isValidInput(column.type, input) ? input : new InvalidInput(input));
          }}
        />
      );
    }
    case 'integer':
      return (
        <NumberInput
          {...common}
          allowDecimal={false}
          thousandSeparator=" "
          value={typeof value === 'number' ? value : text}
          onChange={(v) => onChange(v === '' ? null : typeof v === 'number' ? v : (parseIntegerInput(v) ?? new InvalidInput(v)))}
        />
      );
    case 'decimal':
      return (
        <TextInput
          {...common}
          inputMode="decimal"
          // Non contrôlé : la saisie reste telle que tapée (`12,5`), la valeur est normalisée.
          defaultValue={locale.startsWith('fr') ? text.replace('.', ',') : text}
          onChange={(e) => {
            const input = e.currentTarget.value;
            onChange(input.trim() === '' ? null : (parseDecimalInput(input) ?? new InvalidInput(input)));
          }}
        />
      );
    case 'duration':
      return (
        <TextInput
          {...common}
          description={strings.durationHint}
          defaultValue={typeof value === 'number' ? durationText(value) : text}
          onChange={(e) => {
            const input = e.currentTarget.value;
            onChange(input.trim() === '' ? null : (parseDuration(input) ?? new InvalidInput(input)));
          }}
        />
      );
    case 'text':
      return <Textarea {...common} autosize minRows={3} maxRows={10} value={text} onChange={(e) => onChange(e.currentTarget.value)} />;
    default:
      return <TextInput {...common} maxLength={255} value={text} onChange={(e) => onChange(e.currentTarget.value)} />;
  }
}

/** Champs d'un formulaire, en grille ; les champs larges occupent toute la ligne. */
export function FieldGrid({
  table,
  columns,
  values,
  error,
  onChange,
  cols = 2,
}: {
  table: TableSchema;
  columns: ColumnSchema[];
  values: Json;
  error: (column: ColumnSchema) => string | undefined;
  onChange: (name: string, value: unknown) => void;
  cols?: number;
}) {
  const wide = (c: ColumnSchema) => ['text', 'markdown', 'reference_list'].includes(c.type);
  return (
    <SimpleGrid cols={{ base: 1, md: cols }} spacing="lg">
      {columns.map((c) => (
        <div key={c.name} style={wide(c) ? { gridColumn: '1 / -1' } : undefined}>
          <Field table={table} column={c} value={values[c.name]} error={error(c)} onChange={(value) => onChange(c.name, value)} />
        </div>
      ))}
    </SimpleGrid>
  );
}
