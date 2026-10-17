#include <QApplication>
#include <QtQuickTest/quicktest.h>

// SystemTrayIcon's KDE platform menu creates QWidget instances. Upstream
// qmltestrunner may use QGuiApplication; never disable the production tray.
int main(int argc, char **argv)
{
    QApplication app(argc, argv);
    return quick_test_main(argc, argv, "kde_assistant", nullptr);
}
