import { ActionIcon, Button, Group, Modal, Stack, Tooltip } from '@mantine/core';
import { useDisclosure } from '@mantine/hooks';
import { IconCheck, IconPlus } from '@tabler/icons-react';
import { useState } from 'react';

import { ApiError, type Json } from '../api/client';
import { can, useForge } from '../context';
import { editableColumns, findTable, type TableSchema } from '../schema';
import { useNotify } from './common';
import { clientError, FieldGrid, isEmptyValue } from './fields';

/**
 * Création d'un enregistrement de `target` sans quitter le formulaire en
 * cours (champ de référence) : fenêtre de saisie, puis `onCreated(id)`.
 */
export function CreateRecordButton({ target, onCreated }: { target: string; onCreated: (id: number) => void }) {
  const forge = useForge();
  const [opened, { open, close }] = useDisclosure(false);
  const table = findTable(forge.schema, target);
  if (!table || !can(forge, table, 'create')) return null;
  const title = forge.strings.newRecord(forge.format.tableLabel(table));
  return (
    <>
      <Tooltip label={title}>
        <ActionIcon variant="light" size={36} onClick={open} aria-label={title}>
          <IconPlus size={18} />
        </ActionIcon>
      </Tooltip>
      <Modal opened={opened} onClose={close} title={title} size="lg">
        {opened && (
          <CreateRecordForm
            table={table}
            onCreated={(id) => {
              close();
              onCreated(id);
            }}
          />
        )}
      </Modal>
    </>
  );
}

function CreateRecordForm({ table, onCreated }: { table: TableSchema; onCreated: (id: number) => void }) {
  const forge = useForge();
  const { strings } = forge;
  const notify = useNotify();
  const columns = editableColumns(table);
  const [values, setValues] = useState<Json>(() => Object.fromEntries(columns.map((c) => [c.name, c.default ?? null])));
  const [serverErrors, setServerErrors] = useState<Record<string, string[]>>({});
  const [submitted, setSubmitted] = useState(false);
  const [saving, setSaving] = useState(false);

  const save = async () => {
    setSubmitted(true);
    if (columns.some((c) => clientError(strings, c, values[c.name]))) return;
    setSaving(true);
    try {
      const body = Object.fromEntries(Object.entries(values).filter(([, v]) => !isEmptyValue(v)));
      const record = await forge.client.table(table.name).create(body);
      forge.notifyChange();
      onCreated(record.id as number);
    } catch (error) {
      if (error instanceof ApiError && Object.keys(error.fields).length) {
        setServerErrors(error.fields);
      } else {
        notify.error(error);
      }
    } finally {
      setSaving(false);
    }
  };

  return (
    <Stack>
      <FieldGrid
        table={table}
        columns={columns}
        values={values}
        error={(c) => (submitted ? clientError(strings, c, values[c.name]) : undefined) ?? serverErrors[c.name]?.join(' ')}
        onChange={(name, value) => {
          setValues((v) => ({ ...v, [name]: value }));
          setServerErrors(({ [name]: _, ...rest }) => rest);
        }}
        cols={1}
      />
      <Group justify="flex-end">
        <Button onClick={() => void save()} loading={saving} leftSection={<IconCheck size={18} />} variant="gradient">
          {strings.save}
        </Button>
      </Group>
    </Stack>
  );
}
