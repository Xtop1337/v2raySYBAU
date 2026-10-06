using System.Collections.ObjectModel;
using System.Windows;
using System.Windows.Controls;
using V2RaySybau.Core;
using V2RaySybau.Infrastructure;
using V2RaySybau.Models;
using V2RaySybau.Profiles;
using V2RaySybau.Settings;
using V2RaySybau.Storage;

namespace V2RaySybau.UI;

public partial class MainWindow : Window
{
    private readonly ProfileStore _store = new();
    private readonly ProfileImportService _importer = new();
    private readonly SettingsService _settingsStore = new();
    private readonly WindowsIntegrationService _windows = new();
    private readonly CoreProcessService _core = new();
    private readonly XrayConfigAdapter _configBuilder = new();
    private readonly HttpClient _httpClient = new();

    private readonly ObservableCollection<ConnectionProfile> _profiles = [];
    private readonly ObservableCollection<ConnectionEvent> _events = [];
    private AppSettings _settings = new();
    private bool _isLoading;

    public MainWindow()
    {
        InitializeComponent();
        Loaded += OnLoaded;
        Closing += async (_, e) =>
        {
            e.Cancel = true;
            await DisconnectAsync();
            _settings.AutoConnect = AutoConnect.IsChecked == true;
            await _settingsStore.SaveAsync(_settings);
            await _core.DisposeAsync();
            _httpClient.Dispose();
            Application.Current.Shutdown();
        };
    }

    private async void OnLoaded(object sender, RoutedEventArgs e)
    {
        _isLoading = true;

        // Load settings and profiles
        _settings = await _settingsStore.LoadAsync();
        foreach (var item in await _store.LoadAsync())
            _profiles.Add(item);

        // Bind UI collections
        Profiles.ItemsSource = _profiles;
        Log.ItemsSource = _events;

        // Wire up core events
        _core.EventReceived += (_, ev) =>
        {
            _events.Insert(0, ev);
            if (_events.Count > 1000)
                _events.RemoveAt(_events.Count - 1);
        };

        // Restore UI state
        SystemProxy.IsChecked = _settings.SystemProxy;
        Autostart.IsChecked = _settings.StartWithWindows;
        AutoConnect.IsChecked = _settings.AutoConnect;
        Theme.SelectedIndex = (int)_settings.Theme;

        RefreshGroups();

        _isLoading = false;

        // Auto-connect if enabled and a profile is selected
        if (_settings.AutoConnect && !string.IsNullOrWhiteSpace(_settings.LastSelectedProfileId))
        {
            var lastProfile = _profiles.FirstOrDefault(p => p.Id.ToString() == _settings.LastSelectedProfileId);
            if (lastProfile != null)
            {
                Profiles.SelectedItem = lastProfile;
                await Task.Delay(500);
                await ConnectSelectedAsync();
            }
        }
    }

    private async void Import_Click(object sender, RoutedEventArgs e)
    {
        var text = Clipboard.ContainsText() ? Clipboard.GetText() : "";

        if (string.IsNullOrWhiteSpace(text))
        {
            MessageBox.Show("Скопируйте в буфер обмена ссылку на профиль или подписку.", "Импорт", MessageBoxButton.OK, MessageBoxImage.Information);
            return;
        }

        try
        {
            IReadOnlyList<ConnectionProfile> imported;

            if (text.StartsWith("http://", StringComparison.OrdinalIgnoreCase) || text.StartsWith("https://", StringComparison.OrdinalIgnoreCase))
            {
                imported = await _importer.ImportSubscriptionAsync(text, _httpClient);
            }
            else
            {
                imported = _importer.Import(text);
            }

            if (imported.Count == 0)
            {
                MessageBox.Show("Не удалось разобрать профили. Убедитесь, что скопирована корректная ссылка.", "Импорт", MessageBoxButton.OK, MessageBoxImage.Warning);
                return;
            }

            foreach (var profile in imported)
            {
                if (!_profiles.Any(p => p.Host == profile.Host && p.Port == profile.Port && p.UserId == profile.UserId))
                    _profiles.Add(profile);
            }

            await _store.SaveAsync(_profiles);
            RefreshGroups();

            MessageBox.Show($"Импортировано {imported.Count} профил(ей).", "Импорт", MessageBoxButton.OK, MessageBoxImage.Information);
        }
        catch (Exception ex)
        {
            MessageBox.Show($"Ошибка при импорте: {ex.Message}", "Ошибка", MessageBoxButton.OK, MessageBoxImage.Error);
        }
    }

    private void Export_Click(object sender, RoutedEventArgs e)
    {
        var selected = Profiles.SelectedItems.Cast<ConnectionProfile>().ToList();
        if (selected.Count == 0 && Profiles.SelectedItem is ConnectionProfile p)
            selected.Add(p);

        if (selected.Count == 0)
        {
            MessageBox.Show("Выберите профили для экспорта.", "Экспорт", MessageBoxButton.OK, MessageBoxImage.Information);
            return;
        }

        var dialog = new Microsoft.Win32.SaveFileDialog
        {
            FileName = "profiles.json",
            DefaultExt = ".json",
            Filter = "JSON файлы (*.json)|*.json|Все файлы (*.*)|*.*"
        };

        if (dialog.ShowDialog() == true)
        {
            try
            {
                var json = _store.Export(selected);
                File.WriteAllText(dialog.FileName, json);
                MessageBox.Show("Профили успешно экспортированы.", "Экспорт", MessageBoxButton.OK, MessageBoxImage.Information);
            }
            catch (Exception ex)
            {
                MessageBox.Show($"Ошибка при экспорте: {ex.Message}", "Ошибка", MessageBoxButton.OK, MessageBoxImage.Error);
            }
        }
    }

    private async void Delete_Click(object sender, RoutedEventArgs e)
    {
        if (Profiles.SelectedItem is not ConnectionProfile profile)
            return;

        if (MessageBox.Show($"Удалить профиль «{profile.Name}»?", "Подтверждение", MessageBoxButton.YesNo, MessageBoxImage.Question) == MessageBoxResult.Yes)
        {
            _profiles.Remove(profile);
            await _store.SaveAsync(_profiles);
            RefreshGroups();
        }
    }

    private async void Connect_Click(object sender, RoutedEventArgs e)
    {
        if (_core.IsRunning)
        {
            await DisconnectAsync();
        }
        else
        {
            await ConnectSelectedAsync();
        }
    }

    private async Task ConnectSelectedAsync()
    {
        if (Profiles.SelectedItem is not ConnectionProfile profile)
        {
            MessageBox.Show("Выберите профиль для подключения.", "Ошибка", MessageBoxButton.OK, MessageBoxImage.Warning);
            return;
        }

        if (string.IsNullOrWhiteSpace(profile.Host) || profile.Port <= 0)
        {
            MessageBox.Show("Профиль содержит пустые параметры. Заполните Host и Port.", "Ошибка", MessageBoxButton.OK, MessageBoxImage.Warning);
            return;
        }

        try
        {
            _events.Clear();
            _events.Insert(0, new ConnectionEvent(DateTimeOffset.Now, "info", $"Подключение к {profile.Name}..."));

            var xrayPath = Path.Combine(AppContext.BaseDirectory, "xray.exe");
            if (!File.Exists(xrayPath))
            {
                xrayPath = Path.Combine(AppContext.BaseDirectory, "core", "xray.exe");
            }

            if (!File.Exists(xrayPath))
            {
                MessageBox.Show($"xray.exe не найден. Поместите его в папку приложения или в подпапку 'core'.\n\nИскали в:\n{Path.Combine(AppContext.BaseDirectory, "xray.exe")}\n{Path.Combine(AppContext.BaseDirectory, "core", "xray.exe")}", 
                    "Ошибка", MessageBoxButton.OK, MessageBoxImage.Error);
                return;
            }

            var config = _configBuilder.Build(profile);
            await _core.StartAsync(xrayPath, config);

            ConnectButton.Content = "Отключить";
            ConnectButton.Background = System.Windows.Media.Brushes.Red;

            // Save last selected profile
            _settings.LastSelectedProfileId = profile.Id.ToString();
            await _settingsStore.SaveAsync(_settings);

            _events.Insert(0, new ConnectionEvent(DateTimeOffset.Now, "info", $"✓ Подключено к {profile.Name}"));
        }
        catch (Exception ex)
        {
            _events.Insert(0, new ConnectionEvent(DateTimeOffset.Now, "error", $"✗ Ошибка подключения: {ex.Message}"));
            MessageBox.Show($"Ошибка при подключении:\n{ex.Message}", "Ошибка", MessageBoxButton.OK, MessageBoxImage.Error);
        }
    }

    private async Task DisconnectAsync()
    {
        try
        {
            await _core.StopAsync();
            ConnectButton.Content = "Подключить";
            ConnectButton.Background = System.Windows.Media.Brushes.Green;
            _events.Insert(0, new ConnectionEvent(DateTimeOffset.Now, "info", "Отключено"));
        }
        catch (Exception ex)
        {
            _events.Insert(0, new ConnectionEvent(DateTimeOffset.Now, "error", $"Ошибка отключения: {ex.Message}"));
        }
    }

    private void Profiles_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (Profiles.SelectedItem is ConnectionProfile p)
        {
            NameBox.Text = p.Name;
            HostBox.Text = p.Host;
            PortBox.Text = p.Port.ToString();
            UserIdBox.Text = p.UserId;
            GroupBox.Text = p.Group;
        }
        else
        {
            ClearEditor();
        }
    }

    private async void SaveEditor(object sender, RoutedEventArgs e)
    {
        if (Profiles.SelectedItem is not ConnectionProfile p)
            return;

        p.Name = NameBox.Text;
        p.Host = HostBox.Text;
        if (int.TryParse(PortBox.Text, out var port))
            p.Port = port;
        p.UserId = UserIdBox.Text;
        p.Group = GroupBox.Text;

        await _store.SaveAsync(_profiles);
        RefreshGroups();

        MessageBox.Show("Профиль сохранен.", "Успех", MessageBoxButton.OK, MessageBoxImage.Information);
    }

    private void SystemProxy_Changed(object sender, RoutedEventArgs e)
    {
        if (_isLoading) return;
        _settings.SystemProxy = SystemProxy.IsChecked == true;
        _windows.SetSystemProxy(_settings.SystemProxy);
    }

    private void Autostart_Changed(object sender, RoutedEventArgs e)
    {
        if (_isLoading) return;
        _settings.StartWithWindows = Autostart.IsChecked == true;
        _windows.SetAutostart(_settings.StartWithWindows);
    }

    private void Theme_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (_isLoading) return;
        _settings.Theme = (ThemeMode)Math.Max(0, Theme.SelectedIndex);
        // Theme application logic can be expanded here
    }

    private void GroupFilter_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (_isLoading) return;
        if (GroupFilter.SelectedItem is string group && group != "Все")
        {
            Profiles.ItemsSource = new ObservableCollection<ConnectionProfile>(_profiles.Where(p => p.Group == group));
        }
        else
        {
            Profiles.ItemsSource = _profiles;
        }
    }

    private void RefreshGroups()
    {
        var chosen = GroupFilter.SelectedItem as string;
        var groups = _profiles.Select(x => x.Group).Distinct().ToList();
        GroupFilter.ItemsSource = new[] { "Все" }.Concat(groups).ToList();
        GroupFilter.SelectedItem = chosen ?? "Все";
    }

    private void ClearEditor()
    {
        NameBox.Clear();
        HostBox.Clear();
        PortBox.Clear();
        UserIdBox.Clear();
        GroupBox.Clear();
    }
}
