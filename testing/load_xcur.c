#define _DEFAULT_SOURCE

#include <X11/Xcursor/Xcursor.h>
#include <X11/Xlib.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

int main(int argc, char** argv) {
    char* cursor_name = "left_ptr";
    int size = 32;

    const char* usage_yap = "usage: ./load_xcur [cursor_name=left_ptr] [size=32]\n";

    if (argc > 3) {
        fprintf(stderr, "too many arguments\n");
        fprintf(stderr, "%s", usage_yap);
        return 1;
    }

    if (argc >= 2) {
        cursor_name = argv[1];
    }

    if (argc == 3) {
        size = atoi(argv[2]);

        if (size == 0) {
            fprintf(stderr, "size can't be 0 (or invalid size arg)\n");
            fprintf(stderr, "%s", usage_yap);
            return 1;
        }
    }

    Display* display = XOpenDisplay(NULL);

    if (!display) {
        fprintf(stderr, "`XOpenDisplay()` failed\n");
        return 1;
    }

    Window window = XCreateSimpleWindow(display, DefaultRootWindow(display), 100, 100, 300, 200, 1, 0, 0xffffff);
    XStoreName(display, window, "xcursor test");
    XSelectInput(display, window, ExposureMask | StructureNotifyMask);
    XMapWindow(display, window);
    XcursorImages* image = XcursorFilenameLoadImages(cursor_name, size);

    if (!image) {
        fprintf(stderr, "`XcursorFilenameLoadImages()` failed\n");
        return 1;
    }

    Cursor* frames = malloc(sizeof(Cursor) * (size_t) image->nimage);

    if (!frames) {
        fprintf(stderr, "`malloc()` failed\n");
        return 1;
    }

    for (int i = 0; i < image->nimage; ++i) {
        frames[i] = XcursorImageLoadCursor(display, image->images[i]);
    }

    // might leak but probably not
    int frame = 0;
    while (true) {
        XDefineCursor(display, window, frames[frame]);
        XFlush(display);

        unsigned int delay = image->images[frame]->delay;

        // convert to microseconds
        usleep(delay * 1000);
        frame = (frame + 1) % image->nimage;
    }
}
