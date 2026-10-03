import { Avatar, Badge, Button, Card, Group, PasswordInput, SegmentedControl, Select, Stack, Text, Title, useMantineColorScheme } from '@mantine/core';
import { IconLogout } from '@tabler/icons-react';
import { type FormEvent, useState } from 'react';

import { ApiError, userName } from '../api/client';
import { useForge } from '../context';
import { useNotify } from './common';

/** Nom d'une langue dans cette langue. */
const LANGUAGES: Record<string, string> = {
  fr: 'Français',
  en: 'English',
  de: 'Deutsch',
  es: 'Español',
  it: 'Italiano',
  pt: 'Português',
  nl: 'Nederlands',
};

/** Compte connecté : informations, langue, thème, mot de passe, déconnexion. */
export function AccountPage() {
  const forge = useForge();
  const { strings, user } = forge;
  const { colorScheme, setColorScheme } = useMantineColorScheme();
  return (
    <Stack maw={640} mx="auto">
      <Title order={2}>{strings.account}</Title>
      {user && (
        <Card>
          <Group>
            <Avatar size="lg" radius="xl" variant="gradient">
              {userName(user).slice(0, 2).toUpperCase()}
            </Avatar>
            <div>
              <Text fw={700}>{userName(user)}</Text>
              <Text size="sm" c="dimmed">
                {user.email}
              </Text>
              <Group gap={4} mt={4}>
                {user.roles.map((r) => (
                  <Badge key={r} variant="light" size="sm">
                    {r}
                  </Badge>
                ))}
              </Group>
            </div>
          </Group>
        </Card>
      )}
      <Card>
        <Stack>
          {forge.schema.locales.length > 1 && (
            <Select
              label={strings.language}
              data={forge.schema.locales.map((l) => ({ value: l, label: LANGUAGES[l.split('_')[0]] ?? l }))}
              value={forge.locale}
              onChange={(l) => l && forge.setLocale(l)}
              allowDeselect={false}
            />
          )}
          <div>
            <Text size="sm" fw={500} mb={6}>
              {strings.theme}
            </Text>
            <SegmentedControl
              value={colorScheme}
              onChange={(v) => setColorScheme(v as 'light' | 'dark' | 'auto')}
              data={[
                { value: 'light', label: strings.light },
                { value: 'dark', label: strings.dark },
                { value: 'auto', label: strings.system },
              ]}
            />
          </div>
        </Stack>
      </Card>
      <PasswordForm />
      <Button variant="light" color="red" leftSection={<IconLogout size={18} />} onClick={() => void forge.client.signOut()}>
        {strings.signOut}
      </Button>
    </Stack>
  );
}

function PasswordForm() {
  const forge = useForge();
  const { strings } = forge;
  const notify = useNotify();
  const [current, setCurrent] = useState('');
  const [next, setNext] = useState('');
  const [confirmation, setConfirmation] = useState('');
  const [errors, setErrors] = useState<Record<string, string[]>>({});
  const [saving, setSaving] = useState(false);
  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (next !== confirmation) {
      setErrors({ confirmation: [strings.passwordsDiffer] });
      return;
    }
    setSaving(true);
    setErrors({});
    try {
      await forge.client.put('/api/auth/password', { current_password: current, new_password: next });
      // Le changement ferme toutes les sessions : on en rouvre une.
      await forge.client.signIn(forge.user!.email, next);
      setCurrent('');
      setNext('');
      setConfirmation('');
      notify.success(strings.passwordChanged);
    } catch (e) {
      if (e instanceof ApiError && Object.keys(e.fields).length) setErrors(e.fields);
      else notify.error(e);
    } finally {
      setSaving(false);
    }
  };
  return (
    <Card component="form" onSubmit={submit}>
      <Stack>
        <Title order={4}>{strings.changePassword}</Title>
        <PasswordInput required label={strings.currentPassword} value={current} error={errors.current_password?.join(' ')} onChange={(e) => setCurrent(e.currentTarget.value)} />
        <PasswordInput required label={strings.newPassword} value={next} error={(errors.password ?? errors.new_password)?.join(' ')} onChange={(e) => setNext(e.currentTarget.value)} />
        <PasswordInput required label={strings.confirmPassword} value={confirmation} error={errors.confirmation?.join(' ')} onChange={(e) => setConfirmation(e.currentTarget.value)} />
        <Group justify="flex-end">
          <Button type="submit" loading={saving}>
            {strings.save}
          </Button>
        </Group>
      </Stack>
    </Card>
  );
}
