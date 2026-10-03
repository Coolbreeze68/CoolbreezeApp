/**
 * Affichage et saisie des modèles de champ : couleur, liens (e-mail, web,
 * téléphone), note, Markdown, fichier et image.
 */
import {
  Anchor,
  Button,
  ColorInput,
  ColorSwatch,
  Group,
  Image,
  Input,
  Modal,
  Progress,
  Rating,
  SegmentedControl,
  Stack,
  Text,
  Textarea,
  Typography,
} from '@mantine/core';
import { useDisclosure } from '@mantine/hooks';
import {
  IconExternalLink,
  IconFile,
  IconFileText,
  IconFileTypePdf,
  IconMail,
  IconPhone,
  IconPhoto,
  IconUpload,
  IconX,
} from '@tabler/icons-react';
import { useRef, useState } from 'react';
import Markdown from 'react-markdown';

import { ApiError } from '../api/client';
import { type ForgeFile, isForgeFile } from '../api/file';
import { useForge } from '../context';
import type { FieldProps } from '../customization';
import { CATEGORICAL } from '../palette';
import type { ColumnType } from '../schema';
import { useNotify } from './common';

// -------------------------------------------------------------- affichage

export function ColorValue({ hex }: { hex: string }) {
  return (
    <Group gap={8} wrap="nowrap">
      <ColorSwatch color={hex} size={16} radius="sm" />
      <Text size="sm" ff="monospace">
        {hex}
      </Text>
    </Group>
  );
}

/** Adresse ouverte par le navigateur : `mailto:`, `tel:` ou adresse web. */
export function linkHref(type: ColumnType, value: string): string {
  switch (type) {
    case 'email':
      return `mailto:${value}`;
    case 'phone':
      return `tel:${value.replace(/[^\d+]/g, '')}`;
    default:
      return value;
  }
}

/** E-mail, adresse web ou téléphone, cliquable. */
export function LinkValue({ type, value }: { type: ColumnType; value: string }) {
  const Icon = type === 'email' ? IconMail : type === 'phone' ? IconPhone : IconExternalLink;
  return (
    <Anchor
      href={linkHref(type, value)}
      target={type === 'url' ? '_blank' : undefined}
      rel={type === 'url' ? 'noopener noreferrer' : undefined}
      size="sm"
      onClick={(e) => e.stopPropagation()}
    >
      <Group gap={6} wrap="nowrap" component="span">
        <Icon size={15} />
        {value}
      </Group>
    </Anchor>
  );
}

export function RatingValue({ value, max, size = 'sm' }: { value: number; max: number; size?: 'xs' | 'sm' | 'md' }) {
  return <Rating value={value} count={max} readOnly size={size} aria-label={`${value}/${max}`} />;
}

export function MarkdownValue({ text }: { text: string }) {
  return (
    <Typography>
      <Markdown
        components={{
          a: ({ href, children }) => (
            <a href={href} target="_blank" rel="noopener noreferrer">
              {children}
            </a>
          ),
        }}
      >
        {text}
      </Markdown>
    </Typography>
  );
}

function FileIcon({ contentType }: { contentType: string }) {
  if (contentType === 'application/pdf') return <IconFileTypePdf size={16} />;
  if (contentType.startsWith('image/')) return <IconPhoto size={16} />;
  if (contentType.startsWith('text/')) return <IconFileText size={16} />;
  return <IconFile size={16} />;
}

/** Fichier : icône, nom et taille ; ouvert par le navigateur. */
export function FileValue({ file, link = true }: { file: ForgeFile; link?: boolean }) {
  const { format, client } = useForge();
  const content = (
    <Group gap={6} wrap="nowrap" component="span">
      <FileIcon contentType={file.content_type} />
      <span>{file.name}</span>
      <Text span size="xs" c="dimmed">
        {format.fileSize(file.size)}
      </Text>
    </Group>
  );
  if (!link) return <Text size="sm">{content}</Text>;
  return (
    <Anchor
      href={client.url(file.url)}
      target="_blank"
      rel="noopener noreferrer"
      size="sm"
      onClick={(e) => e.stopPropagation()}
    >
      {content}
    </Anchor>
  );
}

/** Miniature d'une image ; agrandie au clic. */
export function ImageValue({ file, size = 40, zoom = true }: { file: ForgeFile; size?: number; zoom?: boolean }) {
  const { client } = useForge();
  const [opened, { open, close }] = useDisclosure(false);
  const src = client.url(file.url);
  const thumbnail = (
    <Image src={src} alt={file.name} w={size} h={size} fit="cover" radius={size / 8} fallbackSrc="" />
  );
  if (!zoom) return thumbnail;
  return (
    <>
      <button
        type="button"
        onClick={(e) => {
          e.stopPropagation();
          open();
        }}
        style={{ border: 0, padding: 0, background: 'none', cursor: 'zoom-in', width: 'fit-content' }}
        aria-label={file.name}
      >
        {thumbnail}
      </button>
      <Modal opened={opened} onClose={close} title={file.name} size="xl" centered>
        <Image src={src} alt={file.name} radius="md" />
      </Modal>
    </>
  );
}

// ---------------------------------------------------------------- saisie

/** Couleurs proposées : palette de forge, puis quelques neutres. */
const SWATCHES = [...CATEGORICAL.light, '#5b4bd6', '#0f172a', '#64748b', '#ffffff'];

export function ColorField({ column, value, onChange, label, error }: FieldProps & { label: string }) {
  return (
    <ColorInput
      label={label}
      withAsterisk={column.required}
      error={error}
      format="hex"
      swatches={SWATCHES}
      swatchesPerRow={10}
      value={typeof value === 'string' ? value : ''}
      onChange={(hex) => onChange(hex === '' ? null : hex.toLowerCase())}
    />
  );
}

export function RatingField({ column, value, onChange, label, error }: FieldProps & { label: string }) {
  return (
    <Input.Wrapper label={label} withAsterisk={column.required} error={error}>
      <Group gap="xs" mt={6}>
        <Rating
          value={typeof value === 'number' ? value : 0}
          count={column.max ?? 5}
          size="lg"
          onChange={(rating) => onChange(rating)}
        />
        {typeof value === 'number' && !column.required && (
          <Button variant="subtle" size="compact-xs" color="gray" onClick={() => onChange(null)} aria-label="×">
            <IconX size={14} />
          </Button>
        )}
      </Group>
    </Input.Wrapper>
  );
}

/** Texte Markdown : saisie, et aperçu du rendu. */
export function MarkdownField({ column, value, onChange, label, error }: FieldProps & { label: string }) {
  const { strings } = useForge();
  const [mode, setMode] = useState('write');
  const text = typeof value === 'string' ? value : '';
  return (
    <Input.Wrapper
      label={label}
      withAsterisk={column.required}
      error={error}
      inputContainer={(children) => (
        <Stack gap={6} mt={4}>
          <SegmentedControl
            size="xs"
            w="fit-content"
            value={mode}
            onChange={setMode}
            data={[
              { value: 'write', label: strings.write },
              { value: 'preview', label: strings.preview },
            ]}
          />
          {children}
        </Stack>
      )}
    >
      {mode === 'write' ? (
        <Textarea
          autosize
          minRows={5}
          maxRows={16}
          value={text}
          aria-label={label}
          onChange={(e) => onChange(e.currentTarget.value === '' ? null : e.currentTarget.value)}
        />
      ) : (
        <div style={{ border: '1px solid var(--mantine-color-default-border)', borderRadius: 8, padding: 12, minHeight: 80 }}>
          <MarkdownValue text={text} />
        </div>
      )}
    </Input.Wrapper>
  );
}

/** Attribut `accept` du sélecteur de fichiers. */
const acceptAttribute = (type: ColumnType, accept: string[] | undefined) =>
  type === 'image' ? 'image/png,image/jpeg,image/gif,image/webp' : accept?.join(',');

/** Champ fichier ou image : choix, envoi immédiat, remplacement, retrait. */
export function FileField({ table, column, value, onChange, label, error }: FieldProps & { label: string }) {
  const forge = useForge();
  const { strings } = forge;
  const notify = useNotify();
  const input = useRef<HTMLInputElement>(null);
  const [uploading, setUploading] = useState(false);
  const file = isForgeFile(value) ? value : null;
  const image = column.type === 'image';

  const upload = async (picked: File | undefined) => {
    if (!picked) return;
    if (column.max_size && picked.size > column.max_size * 1024 * 1024) {
      notify.error(strings.fileTooLarge(column.max_size));
      return;
    }
    setUploading(true);
    try {
      onChange(await forge.client.table(table.name).upload(column.name, picked));
    } catch (e) {
      const message = e instanceof ApiError ? e.fields[column.name]?.join(' ') : undefined;
      notify.error(message ?? e);
    } finally {
      setUploading(false);
      if (input.current) input.current.value = '';
    }
  };

  return (
    <Input.Wrapper label={label} withAsterisk={column.required} error={error}>
      <Stack gap="xs" mt={6}>
        {file && (image ? <ImageValue file={file} size={120} /> : <FileValue file={file} />)}
        {uploading ? (
          <Stack gap={4}>
            <Progress value={100} animated size="sm" />
            <Text size="xs" c="dimmed">
              {strings.uploading}
            </Text>
          </Stack>
        ) : (
          <Group gap="xs">
            <Button
              variant="light"
              size="xs"
              leftSection={image ? <IconPhoto size={16} /> : <IconUpload size={16} />}
              onClick={() => input.current?.click()}
            >
              {file ? strings.replace : image ? strings.chooseImage : strings.chooseFile}
            </Button>
            {file && !column.required && (
              <Button variant="subtle" color="gray" size="xs" leftSection={<IconX size={16} />} onClick={() => onChange(null)}>
                {strings.remove}
              </Button>
            )}
          </Group>
        )}
        <input
          ref={input}
          type="file"
          hidden
          accept={acceptAttribute(column.type, column.accept)}
          aria-label={label}
          onChange={(e) => void upload(e.currentTarget.files?.[0])}
        />
      </Stack>
    </Input.Wrapper>
  );
}
