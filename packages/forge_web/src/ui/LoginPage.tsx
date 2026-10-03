import { Alert, Avatar, Button, Center, Paper, PasswordInput, Stack, Text, TextInput, Title } from '@mantine/core';
import { IconAlertTriangle, IconLock } from '@tabler/icons-react';
import { type FormEvent, useState } from 'react';

import { ApiError } from '../api/client';
import { useForge } from '../context';
import { useErrorMessage } from './common';

export function LoginPage() {
  const { client, strings, schema } = useForge();
  const message = useErrorMessage();
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setBusy(true);
    setError(null);
    try {
      // La redirection suit l'ouverture de la session.
      await client.signIn(email.trim(), password);
    } catch (e) {
      setError(e instanceof ApiError && e.isUnauthorized ? strings.invalidCredentials : message(e));
      setBusy(false);
    }
  };

  return (
    <Center
      mih="100vh"
      p="md"
      style={{
        background:
          'linear-gradient(135deg, var(--mantine-color-indigo-7) 0%, var(--mantine-color-violet-6) 55%, var(--mantine-color-pink-5) 100%)',
      }}
    >
      <Paper shadow="xl" p="xl" radius="lg" w="100%" maw={420} component="form" onSubmit={submit}>
        <Stack>
          <Stack align="center" gap={6}>
            <Avatar size={56} radius="xl" variant="gradient">
              <IconLock size={28} />
            </Avatar>
            <Title order={2}>{schema.name}</Title>
            <Text c="dimmed" size="sm">
              {strings.signIn}
            </Text>
          </Stack>
          <TextInput
            label={strings.email}
            type="email"
            autoComplete="email"
            required
            value={email}
            onChange={(e) => setEmail(e.currentTarget.value)}
          />
          <PasswordInput
            label={strings.password}
            autoComplete="current-password"
            required
            value={password}
            onChange={(e) => setPassword(e.currentTarget.value)}
          />
          {error && (
            <Alert color="red" icon={<IconAlertTriangle />} py="xs">
              {error}
            </Alert>
          )}
          <Button type="submit" variant="gradient" size="md" loading={busy} fullWidth>
            {strings.signIn}
          </Button>
        </Stack>
      </Paper>
    </Center>
  );
}
