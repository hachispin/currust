#define _DEFAULT_SOURCE

#include <X11/Xcursor/Xcursor.h>
#include <X11/Xlib.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

int main(int argc, char** argv) {
    char* cursor_name = "left_ptr";
    int32_t size = 32;

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

        if (size <= 0) {
            fprintf(stderr, "size must be greater than zero (or invalid size arg)\n");
            fprintf(stderr, "%s", usage_yap);
            return 1;
        }
    }

    Display* display = XOpenDisplay(NULL);

    if (!display) {
        fprintf(stderr, "XOpenDisplay() failed\n");
        return 1;
    }

    const int32_t WINDOW_X = 100;
    const int32_t WINDOW_Y = 100;
    const uint32_t WIDTH = 600;
    const uint32_t HEIGHT = 400;
    const uint32_t BORDER_WIDTH = 1;
    const uint64_t BORDER_COLOR = 0;            // black
    const uint64_t BACKGROUND_COLOR = 0xFFFFFF; // white

    // create window to display cursor in
    Window window = XCreateSimpleWindow(
        display, DefaultRootWindow(display),
        WINDOW_X, WINDOW_Y,
        WIDTH, HEIGHT,
        BORDER_WIDTH, BORDER_COLOR,
        BACKGROUND_COLOR);

    if (!window) {
        fprintf(stderr, "XCreateSimpleWindow() failed\n");
        return 1;
    }

    // set title and do some weird stuff
    XStoreName(display, window, "Xcursor test (currust)");
    XSelectInput(display, window, ExposureMask | StructureNotifyMask);
    XMapWindow(display, window);

    XcursorImages* image = XcursorFilenameLoadImages(cursor_name, size);

    if (!image) {
        fprintf(stderr, "XcursorFilenameLoadImages() failed\n");
        return 1;
    }

    // i.e, length of frames array
    const size_t num_frames = (size_t) image->nimage;

    if (num_frames == 0) {
        fprintf(stderr, "no frames (nimage == 0)\n");
        return 1;
    }

    Cursor* frames = malloc(sizeof(Cursor) * num_frames);

    if (!frames) {
        fprintf(stderr, "malloc() failed\n");
        return 1;
    }

    for (size_t i = 0; i < num_frames; ++i) {
        frames[i] = XcursorImageLoadCursor(display, image->images[i]);
    }

    // current index to frames array
    size_t frame = 0;
    while (true) {
        XDefineCursor(display, window, frames[frame]);
        XFlush(display);

        uint32_t delay = image->images[frame++]->delay;
        frame = frame % num_frames;

        // sleep before next frame, convert to microseconds
        usleep(delay * 1000);
    }

    // this might leak but oh well. no frees for you
}
