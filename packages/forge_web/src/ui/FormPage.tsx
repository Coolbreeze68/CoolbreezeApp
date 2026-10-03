import { Alert, Button, Card, Group, Stack, Text, Title } from '@mantine/core';
import { IconArrowLeft, IconCheck } from '@tabler/icons-react';
import { useEffect, useRef, useState } from 'react';
import { useBlocker, useNavigate, useSearchParams } from 'react-router';
import { modals } from '@mantine/modals';

import { ApiError, type Json } from '../api/client';
import { useForge } from '../context';
import { paths } from '../paths';
import { editableColumns, type TableSchema } from '../schema';
import { ErrorView, Loading, useNotify } from './common';
import { clientError, FieldGrid, isEmptyValue, parseInitialValue } from './fields';

const same = (a: unknown, b: unknown) => JSON.stringify(a ?? null) === JSON.stringify(b ?? null);

/** Création (`id` absent) ou modification d'un enregistrement. */
export function FormPage({ table, id }: { table: TableSchema; id?: number }) {
  const forge = useForge();
  const { strings, format } = forge;
  const navigate = useNavigate();
  const notify = useNotify();
  const [params] = useSearchParams();
  const creating = id === undefined;
  const columns = editableColumns(table);
  const [original, setOriginal] = useState<Json | null>(() =>
    creating
      ? Object.fromEntries(
          columns.map((c) => {
            const initial = params.get(c.name);
            return [c.name, initial !== null ? parseInitialValue(c, initial) : (c.default ?? null)];
          }),
        )
      : null,
  );
  const [values, setValues] = useState<Json>(original ?? {});
  const [loadError, setLoadError] = useState<unknown>(null);
  const [serverErrors, setServerErrors] = useState<Record<string, string[]>>({});
  const [submitted, setSubmitted] = useState(false);
  const [saving, setSaving] = useState(false);
  /** Enregistré : la navigation qui suit n'est plus bloquée (posé avant elle). */
  const saved = useRef(false);

  useEffect(() => {
    if (creating) return;
    let current = true;
    forge.client
      .table(table.name)
      .read(id)
      .then((record) => {
        if (!current) return;
        const loaded = Object.fromEntries(columns.map((c) => [c.name, record[c.name] ?? null]));
        setOriginal(loaded);
        setValues(loaded);
      })
      .catch((e: unknown) => current && setLoadError(e));
    return () => {
      current = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [table.name, id]);

  /** Colonnes modifiées ; pour une création, toutes les valeurs renseignées. */
  const changes = () =>
    Object.fromEntries(
      Object.entries(values).filter(([k, v]) => (creating ? !isEmptyValue(v) : !same(v, original?.[k]))),
    );
  const dirty = original !== null && Object.keys(changes()).length > 0;

  // Quitter avec des modifications non enregistrées : confirmation.
  const blocker = useBlocker(() => dirty && !saved.current);
  useEffect(() => {
    if (blocker.state !== 'blocked') return;
    modals.openConfirmModal({
      title: strings.discard,
      children: <Text size="sm">{strings.discardChanges}</Text>,
      labels: { confirm: strings.discard, cancel: strings.cancel },
      confirmProps: { color: 'red' },
      onConfirm: () => blocker.proceed(),
      onCancel: () => blocker.reset(),
    });
  }, [blocker, strings]);

  const save = async () => {
    setSubmitted(true);
    if (columns.some((c) => clientError(strings, c, values[c.name]))) {
      notify.error(strings.fixErrors);
      return;
    }
    const body = changes();
    if (!creating && Object.keys(body).length === 0) {
      navigate(paths.record(table.name, id));
      return;
    }
    setSaving(true);
    try {
      const records = forge.client.table(table.name);
      const record = creating ? await records.create(body) : await records.update(id, body);
      saved.current = true;
      forge.notifyChange();
      navigate(paths.record(table.name, record.id as number), { replace: true });
    } catch (error) {
      if (error instanceof ApiError && Object.keys(error.fields).length) {
        setServerErrors(error.fields);
        notify.error(strings.fixErrors);
      } else {
        notify.error(error);
      }
    } finally {
      setSaving(false);
    }
  };

  if (loadError) return <ErrorView error={loadError} />;
  if (original === null) return <Loading />;
  const known = new Set(columns.map((c) => c.name));
  const otherErrors = Object.entries(serverErrors).filter(([k]) => !known.has(k));

  return (
    <Stack maw={900} mx="auto" gap="lg">
      <Group justify="space-between">
        <Group gap="sm">
          <Button variant="subtle" px="xs" onClick={() => navigate(-1)} aria-label="←">
            <IconArrowLeft size={20} />
          </Button>
          <Title order={2}>
            {creating ? strings.newRecord(format.tableLabel(table)) : strings.editRecord(format.tableLabel(table))}
          </Title>
        </Group>
        <Button onClick={() => void save()} loading={saving} leftSection={<IconCheck size={18} />} variant="gradient">
          {strings.save}
        </Button>
      </Group>
      {otherErrors.length > 0 && (
        <Alert color="red">
          {otherErrors.map(([k, messages]) => (
            <div key={k}>
              {k} : {messages.join(', ')}
            </div>
          ))}
        </Alert>
      )}
      <Card p="lg">
        <FieldGrid
          table={table}
          columns={columns}
          values={values}
          error={(c) => (submitted ? clientError(strings, c, values[c.name]) : undefined) ?? serverErrors[c.name]?.join(' ')}
          onChange={(name, value) => {
            setValues((v) => ({ ...v, [name]: value }));
            setServerErrors(({ [name]: _, ...rest }) => rest);
          }}
        />
      </Card>
    </Stack>
  );
}
