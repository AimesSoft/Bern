#include <QApplication>
#include <QLabel>

int main(int argc, char** argv) {
    QApplication app(argc, argv);
    QLabel label("Hello, World!");
    label.setAlignment(Qt::AlignCenter);
    label.resize(480, 320);
    label.show();
    return app.exec();
}
