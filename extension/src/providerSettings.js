export const SUPPORTED_PROVIDERS = [
    {id: 'codex', title: 'Codex'},
    {id: 'claude', title: 'Claude'},
];

const DEFAULT_PROVIDER_ID = 'codex';
const SOURCE_VALUES = ['auto', 'upstream_cli', 'off'];

export function effectiveProviderSettings(settings) {
    const configured = plainObject(settings?.providers) ? settings.providers : {};
    const hasConfiguredProviders = Object.keys(configured).length > 0;
    const result = {};

    for (const provider of SUPPORTED_PROVIDERS) {
        const providerSettings = plainObject(configured[provider.id]) ? configured[provider.id] : {};
        result[provider.id] = {
            enabled: booleanOrDefault(
                providerSettings.enabled,
                hasConfiguredProviders ? false : provider.id === DEFAULT_PROVIDER_ID
            ),
            preferredSourceAdapter: normalizePreferredSourceAdapter(providerSettings.preferredSourceAdapter),
            allowBrowserImport: false,
            allowCliFallback: booleanOrDefault(providerSettings.allowCliFallback, true),
        };
    }

    return result;
}

export function buildProviderSettingsPatch(settings, change) {
    const providers = effectiveProviderSettings(settings);
    const providerId = change?.providerId;
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
