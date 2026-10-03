import { type ComboboxItem, MultiSelect, Select } from '@mantine/core';
import { useDebouncedValue } from '@mantine/hooks';
import { type CSSProperties, useEffect, useState } from 'react';

import type { Json } from '../api/client';
import { useForge } from '../context';
import { findTable } from '../schema';

/** Enregistrements de `target` correspondant à la recherche, en options. */
function useOptions(target: string, selected: number[]) {
  const forge = useForge();
  const [search, setSearch] = useState('');
  const [debounced] = useDebouncedValue(search, 250);
  const [options, setOptions] = useState<ComboboxItem[]>([]);
  const [known, setKnown] = useState<Record<string, string>>({});
  const table = findTable(forge.schema, target);

  useEffect(() => {
    if (!table) return;
    let current = true;
    forge.client
      .table(target)
      .list({ search: debounced, perPage: 20 })
      .then((listing) => {
        if (!current) return;
        setOptions(
          listing.items.map((r: Json) => ({
            value: String(r.id),
            label: forge.format.title(table, r),
          })),
        );
      })
      .catch(() => current && setOptions([]));
    return () => {
      current = false;
    };
  }, [forge, table, target, debounced]);

  // Intitulés des valeurs choisies absentes des options affichées.
  const selectedKey = selected.join(',');
  useEffect(() => {
    let current = true;
    for (const id of selected) {
      void forge.titles.title(target, id).then(
        (title) => current && setKnown((k) => (k[id] === title ? k : { ...k, [id]: title })),
      );
    }
    return () => {
      current = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [forge.titles, target, selectedKey]);

  const data = [
    ...options,
    ...selected
      .filter((id) => !options.some((o) => o.value === String(id)))
      .map((id) => ({ value: String(id), label: known[id] ?? `#${id}` })),
  ];
  return { data, search, setSearch };
}

interface ReferenceSelectProps {
  target: string;
  value: number | null;
  onChange: (id: number | null) => void;
  label?: string;
  error?: string;
  required?: boolean;
  placeholder?: string;
  style?: CSSProperties;
}

/** Choix d'un enregistrement de `target`, avec recherche. */
export function ReferenceSelect({ target, value, onChange, ...props }: ReferenceSelectProps) {
  const { data, search, setSearch } = useOptions(target, value === null ? [] : [value]);
  return (
    <Select
      {...props}
      data={data}
      value={value === null ? null : String(value)}
      onChange={(v) => onChange(v === null ? null : Number(v))}
      searchable
      searchValue={search}
      onSearchChange={setSearch}
      filter={({ options }) => options}
      clearable={!props.required}
      nothingFoundMessage="—"
    />
  );
}

interface ReferenceMultiSelectProps {
  target: string;
  value: number[];
  onChange: (ids: number[]) => void;
  label?: string;
  error?: string;
  style?: CSSProperties;
}

/** Choix de plusieurs enregistrements de `target`, avec recherche. */
export function ReferenceMultiSelect({ target, value, onChange, ...props }: ReferenceMultiSelectProps) {
  const { data, search, setSearch } = useOptions(target, value);
  return (
    <MultiSelect
      {...props}
      data={data}
      value={value.map(String)}
      onChange={(v) => onChange(v.map(Number))}
      searchable
      searchValue={search}
      onSearchChange={setSearch}
      filter={({ options }) => options}
      clearable
      nothingFoundMessage="—"
    />
  );
}
