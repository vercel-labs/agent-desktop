// WinForms target for the scroll foreground probe: one ListBox with 500 rows.
using System;
using System.Windows.Forms;

static class SfpForms
{
    [STAThread]
    static void Main()
    {
        Application.EnableVisualStyles();
        Form form = new Form();
        form.Text = "SFP Forms";
        form.StartPosition = FormStartPosition.Manual;
        form.Left = 200;
        form.Top = 120;
        form.Width = 700;
        form.Height = 500;
        ListBox list = new ListBox();
        list.Name = "lstRows";
        list.Left = 12;
        list.Top = 12;
        list.Width = 300;
        list.Height = 300;
        for (int i = 1; i <= 500; i++)
        {
            list.Items.Add("Row " + i.ToString("000"));
        }
        Label label = new Label();
        label.Name = "lblInfo";
        label.Text = "scroll foreground probe target";
        label.Left = 330;
        label.Top = 12;
        label.Width = 300;
        form.Controls.Add(list);
        form.Controls.Add(label);
        Application.Run(form);
    }
}
