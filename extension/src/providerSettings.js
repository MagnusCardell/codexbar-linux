export const DEFAULT_PROVIDER_IDS = ['codex', 'claude'];

export const SUPPORTED_PROVIDERS = [
    {id: 'codex', title: 'Codex'},
    {id: 'openai', title: 'OpenAI'},
    {id: 'claude', title: 'Claude'},
    {id: 'cursor', title: 'Cursor'},
    {id: 'opencode', title: 'OpenCode'},
    {id: 'opencodego', title: 'OpenCode Go'},
    {id: 'alibaba-coding-plan', title: 'Alibaba Coding Plan'},
    {id: 'factory', title: 'Factory'},
    {id: 'gemini', title: 'Gemini'},
    {id: 'antigravity', title: 'Antigravity'},
    {id: 'copilot', title: 'GitHub Copilot'},
    {id: 'zai', title: 'Z.ai'},
    {id: 'minimax', title: 'MiniMax'},
    {id: 'manus', title: 'Manus'},
    {id: 'kimi', title: 'Kimi'},
    {id: 'kilo', title: 'Kilo'},
    {id: 'kiro', title: 'Kiro'},
    {id: 'vertexai', title: 'Vertex AI'},
    {id: 'augment', title: 'Augment'},
    {id: 'jetbrains', title: 'JetBrains'},
    {id: 'kimik2', title: 'Kimi K2'},
    {id: 'amp', title: 'Amp'},
    {id: 'ollama', title: 'Ollama'},
    {id: 'synthetic', title: 'Synthetic'},
    {id: 'warp', title: 'Warp'},
    {id: 'openrouter', title: 'OpenRouter'},
    {id: 'windsurf', title: 'Windsurf'},
    {id: 'perplexity', title: 'Perplexity'},
    {id: 'mimo', title: 'Mimo'},
    {id: 'doubao', title: 'Doubao'},
    {id: 'abacusai', title: 'Abacus.AI'},
    {id: 'mistral', title: 'Mistral'},
    {id: 'deepseek', title: 'DeepSeek'},
    {id: 'codebuff', title: 'Codebuff'},
    {id: 'crof', title: 'Crof'},
    {id: 'venice', title: 'Venice'},
    {id: 'commandcode', title: 'CommandCode'},
    {id: 'stepfun', title: 'StepFun'},
];

export const DEFAULT_PROVIDER_SETTINGS = {
    codex: {
        enabled: true,
        preferredSourceAdapter: 'auto',
        allowBrowserImport: false,
        allowCliFallback: true,
    },
    claude: {
        enabled: true,
        preferredSourceAdapter: 'auto',
        allowBrowserImport: false,
        allowCliFallback: true,
    },
};

export const DEFAULT_DAEMON_SETTINGS = {
    schemaVersion: 1,
    refresh: {
        intervalSeconds: 300,
        startupRefresh: true,
        allowStaleCacheFallback: true,
    },
    providers: DEFAULT_PROVIDER_SETTINGS,
    browserImport: {
        enabled: false,
        policy: 'off',
        profileIdAllowlist: [],
        domainAllowlistMode: 'provider_required_only',
    },
    diagnostics: {
        verbosity: 'normal',
        keepRedactedArtifacts: false,
    },
};

const SOURCE_VALUES = ['auto', 'upstream_cli', 'off'];
const PROVIDER_ID_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$/;
const PSEUDO_PROVIDER_IDS = new Set(['all', 'both']);

export function providerCatalog(...sources) {
    return catalogFromSources({
        includeKnownProviders: true,
        includeProviderInventory: true,
        sources,
    });
}

export function runtimeProviderCatalog(...sources) {
    return catalogFromSources({
        includeKnownProviders: false,
        includeProviderInventory: false,
        sources,
    });
}

export function effectiveProviderSettings(settings, providers = runtimeProviderCatalog(settings)) {
    const configured = plainObject(settings?.providers) ? settings.providers : {};
    const hasConfiguredProviders = Object.keys(configured).length > 0;
    const result = {};

    for (const provider of providers) {
        const providerSettings = plainObject(configured[provider.id]) ? configured[provider.id] : {};
        result[provider.id] = {
            enabled: booleanOrDefault(
                providerSettings.enabled,
                hasConfiguredProviders ? false : DEFAULT_PROVIDER_IDS.includes(provider.id)
            ),
            preferredSourceAdapter: normalizePreferredSourceAdapter(providerSettings.preferredSourceAdapter),
            allowBrowserImport: false,
            allowCliFallback: booleanOrDefault(providerSettings.allowCliFallback, true),
        };
    }

    return result;
}

export function buildProviderSettingsPatch(settings, change) {
    const providerId = change?.providerId;
    const providers = effectiveProviderSettings(
        settings,
        runtimeProviderCatalog(settings, providerId ? [{id: providerId}] : [])
    );
    if (!Object.prototype.hasOwnProperty.call(providers, providerId))
        return {schemaVersion: 1, providers};

    if (typeof change.enabled === 'boolean')
        providers[providerId].enabled = change.enabled;
    if (typeof change.preferredSourceAdapter === 'string')
        providers[providerId].preferredSourceAdapter = normalizePreferredSourceAdapter(change.preferredSourceAdapter);

    return {
        schemaVersion: 1,
        providers,
    };
}

function catalogFromSources({includeKnownProviders, includeProviderInventory, sources}) {
    const providers = [];
    const seen = new Set();
    const add = (id, title = null) => {
        if (!isSafeProviderId(id) || seen.has(id))
            return;
        providers.push({
            id,
            title: safeTitle(title) || knownProviderTitle(id) || titleFromProviderId(id),
        });
        seen.add(id);
    };

    if (includeKnownProviders) {
        for (const provider of SUPPORTED_PROVIDERS)
            add(provider.id, provider.title);
    } else {
        for (const providerId of DEFAULT_PROVIDER_IDS)
            add(providerId, knownProviderTitle(providerId));
    }

    for (const source of sources)
        collectProviderIds(source, add, includeProviderInventory);

    return providers;
}

function collectProviderIds(source, add, includeProviderInventory) {
    if (!source)
        return;
    if (Array.isArray(source)) {
        for (const value of source)
            collectProviderIds(value, add, includeProviderInventory);
        return;
    }
    if (!plainObject(source))
        return;

    if (typeof source.id === 'string') {
        add(source.id, source.title ?? source.displayName ?? null);
        return;
    }
    if (typeof source.provider === 'string')
        add(source.provider, source.displayName ?? source.title ?? null);

    const providers = source.providers;
    if (plainObject(providers)) {
        for (const providerId of Object.keys(providers))
            add(providerId);
    } else if (Array.isArray(providers)) {
        for (const provider of providers)
            collectProviderIds(provider, add, includeProviderInventory);
    }

    if (!includeProviderInventory)
        return;

    const upstreamCli = source.upstreamCli
        ?? source.daemon?.upstreamCli
        ?? null;
    if (Array.isArray(upstreamCli?.providerInventory)) {
        for (const provider of upstreamCli.providerInventory)
            collectProviderIds(provider, add, includeProviderInventory);
    }
}

export function titleFromProviderId(providerId) {
    return String(providerId ?? '')
        .replace(/[._:-]+/g, ' ')
        .replace(/\b\w/g, letter => letter.toUpperCase());
}

function knownProviderTitle(providerId) {
    return SUPPORTED_PROVIDERS.find(provider => provider.id === providerId)?.title ?? null;
}

function normalizePreferredSourceAdapter(value) {
    if (value === 'linux_web')
        return 'upstream_cli';
    return SOURCE_VALUES.includes(value) ? value : 'auto';
}

function booleanOrDefault(value, fallback) {
    return typeof value === 'boolean' ? value : fallback;
}

function plainObject(value) {
    return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function isSafeProviderId(value) {
    return typeof value === 'string'
        && PROVIDER_ID_PATTERN.test(value)
        && !PSEUDO_PROVIDER_IDS.has(value);
}

function safeTitle(value) {
    if (typeof value !== 'string')
        return '';
    const trimmed = value.trim();
    return trimmed.length > 0 && trimmed.length <= 80 ? trimmed : '';
}
