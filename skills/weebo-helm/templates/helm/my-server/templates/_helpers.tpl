{{/* source: batlehub@20879e5 (helm/batlehub/templates/_helpers.tpl), trimmed. */}}

{{- define "my-server.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{- define "my-server.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{- define "my-server.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{- define "my-server.labels" -}}
helm.sh/chart: {{ include "my-server.chart" . }}
{{ include "my-server.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{- define "my-server.selectorLabels" -}}
app.kubernetes.io/name: {{ include "my-server.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{- define "my-server.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "my-server.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
Render config.toml by serialising .Values.config as TOML.

Helm loads all YAML numbers as float64 internally, so toToml would emit
"8080.0" for port — which serde rejects for integer fields. Coerce every
integer field back to int/int64 before calling toToml; add a line here for
each integer key the config grows.
*/}}
{{- define "my-server.config" -}}
{{- $c := .Values.config | deepCopy -}}
{{- if $c.server -}}
  {{- $_ := set $c.server "port" (int $c.server.port) -}}
{{- end -}}
{{- $c | toToml -}}
{{- end }}
