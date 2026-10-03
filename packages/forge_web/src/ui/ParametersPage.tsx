import { ActionIcon, Button, Card, Group, Modal, Stack, Table, Text } from '@mantine/core';
import { IconPencil } from '@tabler/icons-react';
import { useState } from 'react';

import { ApiError, type Json } from '../api/client';
import { useForge } from '../context';
import { useAsync } from '../hooks';
import type { ColumnSchema, Parameter, TableSchema } from '../schema';
import { ErrorView, Loading, PageHeader, useErrorMessage } from './common';
import { Field, InvalidInput } from './fields';

/** Table fictive portant les paramètres, pour réutiliser les champs. */
const PARAMETERS: TableSchema = { name: 'parameters', columns: [] };

const columnOf = (p: Parameter): ColumnSchema => ({ name: p.name, type: p.type, label: p.label });

/** Paramètres de l'application (administrateurs) ; un changement recalcule les formules qui les lisent. */
export function ParametersPage() {
  const forge = useForge();
  const { strings, format } = forge;
  const [editing, setEditing] = useState<Parameter | null>(null);
  const values = useAsync(async () => {
    const list = (await forge.client.get('/api/parameters')) as Json[];
    return Object.fromEntries(list.map((p) => [p.name as string, p.value]));
  }, []);
  if (values.error) return <ErrorView error={values.error} onRetry={values.reload} />;
  if (!values.data) return <Loading />;
  return (
    <Stack maw={900} mx="auto">
      <PageHeader title={strings.parameters} />
      <Card p={0}>
        <Table verticalSpacing="md" horizontalSpacing="lg" highlightOnHover>
          <Table.Tbody>
            {(forge.schema.parameters ?? []).map((p) => (
              <Table.Tr key={p.name}>
                <Table.Td>
                  <Text fw={600}>{format.label(p.label, p.name)}</Text>
                </Table.Td>
                <Table.Td>{format.format(PARAMETERS, columnOf(p), values.data![p.name])}</Table.Td>
                <Table.Td w={60}>
                  <ActionIcon variant="light" onClick={() => setEditing(p)} aria-label={strings.edit}>
                    <IconPencil size={16} />
                  </ActionIcon>
                </Table.Td>
              </Table.Tr>
            ))}
          </Table.Tbody>
        </Table>
      </Card>
      {editing && (
        <ParameterModal
          parameter={editing}
          value={values.data[editing.name]}
          onClose={() => setEditing(null)}
          onSaved={() => {
            setEditing(null);
            forge.notifyChange();
          }}
        />
      )}
    </Stack>
  );
}

function ParameterModal({
  parameter,
  value: initial,
  onClose,
  onSaved,
}: {
  parameter: Parameter;
  value: unknown;
  onClose: () => void;
  onSaved: () => void;
}) {
  const forge = useForge();
  const message = useErrorMessage();
  const [value, setValue] = useState(initial);
  const [error, setError] = useState<string>();
  const [saving, setSaving] = useState(false);
  const save = async () => {
    if (value instanceof InvalidInput) {
      setError(forge.strings.invalidValue);
      return;
    }
    setSaving(true);
    try {
      await forge.client.put(`/api/parameters/${parameter.name}`, { value });
      onSaved();
    } catch (e) {
      setError(
        e instanceof ApiError && Object.keys(e.fields).length
          ? Object.values(e.fields).flat().join(' ')
          : message(e),
      );
      setSaving(false);
    }
  };
  return (
    <Modal opened onClose={onClose} title={forge.format.label(parameter.label, parameter.name)} centered>
      <Stack>
        <Field
          table={PARAMETERS}
          column={columnOf(parameter)}
          value={value}
          error={error}
          onChange={(v) => {
            setValue(v);
            setError(undefined);
          }}
        />
        <Group justify="flex-end">
          <Button variant="default" onClick={onClose}>
            {forge.strings.cancel}
          </Button>
          <Button onClick={() => void save()} loading={saving}>
            {forge.strings.save}
          </Button>
        </Group>
      </Stack>
    </Modal>
  );
}
