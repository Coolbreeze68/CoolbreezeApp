import { List, Text } from '@mantine/core';
import { modals } from '@mantine/modals';

import { ApiError } from '../api/client';
import type { ListQuery } from '../api/query';
import type { Forge } from '../context';
import type { TableSchema } from '../schema';
import { dateToJson } from '../values';

type Notify = { success: (text: string) => void; error: (error: unknown) => void };

/**
 * Exporte en CSV les enregistrements correspondant à `query` (séparateur `;`
 * dans les langues où la virgule est décimale, comme le veulent les tableurs).
 */
export async function exportCsv(forge: Forge, table: TableSchema, query: ListQuery, notify: Notify) {
  const decimalComma = new Intl.NumberFormat(forge.locale.replace('_', '-')).format(1.5).includes(',');
  try {
    const blob = await forge.client.table(table.name).export(query, decimalComma ? ';' : ',');
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = url;
    link.download = `${table.name}-${dateToJson(new Date())}.csv`;
    link.click();
    URL.revokeObjectURL(url);
    notify.success(forge.strings.exported);
  } catch (error) {
    notify.error(error);
  }
}

/** Importe un fichier CSV ; tout ou rien. Retourne vrai si l'import a eu lieu. */
export async function importCsv(forge: Forge, table: TableSchema, file: File, notify: Notify) {
  const s = forge.strings;
  try {
    const report = await forge.client.table(table.name).import(file);
    forge.notifyChange();
    notify.success(s.imported(report.created, report.updated));
  } catch (error) {
    if (!(error instanceof ApiError) || error.lines.length === 0) {
      notify.error(error);
      return;
    }
    modals.open({
      title: s.importRejected,
      size: 'lg',
      children: (
        <List spacing="xs">
          {error.lines.map((line) => (
            <List.Item key={line.line}>
              <Text fw={600} span>
                {s.line(line.line)}
              </Text>{' '}
              {line.message}
              {Object.entries(line.fields).map(([field, messages]) => (
                <Text key={field} size="sm" c="dimmed">
                  {field} : {messages.join(', ')}
                </Text>
              ))}
            </List.Item>
          ))}
        </List>
      ),
    });
  }
}
