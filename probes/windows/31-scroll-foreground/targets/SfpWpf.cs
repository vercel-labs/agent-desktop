// WPF target for the scroll foreground probe: one virtualized ListBox with 500 rows.
using System;
using System.Collections.Generic;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;

static class SfpWpf
{
    [STAThread]
    static void Main()
    {
        List<string> rows = new List<string>();
        for (int i = 1; i <= 500; i++)
        {
            rows.Add("Row " + i.ToString("000"));
        }
        ListBox list = new ListBox();
        list.Name = "lstRows";
        AutomationProperties.SetAutomationId(list, "lstRows");
        list.Width = 300;
        list.Height = 300;
        list.HorizontalAlignment = HorizontalAlignment.Left;
        VirtualizingStackPanel.SetIsVirtualizing(list, true);
        VirtualizingStackPanel.SetVirtualizationMode(list, VirtualizationMode.Recycling);
        ScrollViewer.SetCanContentScroll(list, true);
        list.ItemsSource = rows;
        TextBlock label = new TextBlock();
        label.Text = "scroll foreground probe target";
        StackPanel panel = new StackPanel();
        panel.Margin = new Thickness(12);
        panel.Children.Add(label);
        panel.Children.Add(list);
        Window window = new Window();
        window.Title = "SFP WPF";
        window.WindowStartupLocation = WindowStartupLocation.Manual;
        window.Left = 220;
        window.Top = 140;
        window.Width = 700;
        window.Height = 500;
        window.Content = panel;
        new Application().Run(window);
    }
}
