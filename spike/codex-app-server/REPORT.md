# S4Quota — relatório do spike do Codex App Server

Data: 2026-09-19  
Plataforma: Windows  
Resultado: **GO COM RESSALVAS**

## Escopo e segurança

Este spike validou somente a integração com o Codex App Server. Ele não criou
aplicativo Tauri, projeto React, UI, Design System ou provider de produção.

O harness não leu `auth.json`, tokens, caches de autenticação ou endpoints HTTP
privados. Respostas brutas de conta e quota existiram somente na memória do
processo. Diagnósticos persistidos ou exibidos omitem e-mails, IDs de conta,
percentuais exatos, timestamps exatos de reset, tokens, prompts e saída do modelo.

O ambiente não possuía `rustc` ou `cargo` na execução original. Por isso, o
spike isolado usa Node.js sem dependências externas. O resultado desta etapa é
preservado sem alterações de conclusão.

## Resultados efetivamente testados

- Descoberta nativa e shim npm `.cmd`: passaram.
- Handshake `initialize`/`initialized`: passou.
- `account/read`: passou com conta ChatGPT e campos de plano/autenticação presentes.
- `account/rateLimits/read`: passou com snapshot multi-bucket.
- Janela de 300 minutos (5h): presente, percentual válido e reset futuro.
- Janela de 10.080 minutos (semanal): presente, percentual válido e reset futuro.
- Refresh explícito após atividade externa: passou.
- Observação passiva de 20 segundos: nenhuma notificação de rate limit.
- Atividade em outro processo: turno mínimo read-only passou; `account/updated` chegou,
  mas `account/rateLimits/updated` não chegou em cinco segundos; refresh explícito passou.
- Método inexistente: servidor retornou `-32600`; capability deve ser provada por sucesso.
- Shutdown real gracioso: passou.
- Testes sintéticos finais: 16 passaram, 0 falharam.

## Cobertura sintética

Frames fragmentados e múltiplos, JSON inválido/grande, correlação concorrente,
timeouts, stderr limitado/drenado, crash no handshake, shutdown gracioso,
encerramento forçado de árvore Windows, ausência de processo neto, discovery
inválido, wrapper `.cmd` e sanitização de credenciais/identidade.

## Não reproduzido

Logout/token expirado, outage de rede, sleep/resume, relógio/monitores,
outras contas/workspaces, quota esgotada, créditos/spend control, observação
prolongada de notificações e versões antigas/futuras. Job Object Rust também
não foi testado porque o toolchain não estava disponível na execução original;
o harness validou `taskkill /T /F` como fallback.

## Recomendação

**GO COM RESSALVAS** para usar o App Server como fonte primária: os métodos
necessários funcionam no Windows e expõem as janelas reais. Ressalvas:

1. Compatibilidade por capability probe, não por versão.
2. Polling obrigatório; notificações não são fonte autoritativa.
3. Preferir executável nativo e registrar origem/versão; `.cmd` requer launcher controlado.
4. stdout somente protocolo, stderr drenado/limitado e diagnósticos sanitizados.
5. Provider Rust deve usar backoff, Job Object e prevenção de órfãos.
6. Não usar scraping de arquivos ou endpoints privados como fallback.

Nenhuma conclusão do spike foi alterada. Nenhuma etapa posterior foi iniciada.
