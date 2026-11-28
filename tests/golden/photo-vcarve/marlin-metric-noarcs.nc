(Summary, written by PutiaCNC)
(Spindle (!):            Not run by the program: it stops -M0- to ask you to start it at the)
(                        speed shown, and again before a tool change and at the end to stop it.)
(                        There is no work coordinate system: jog to the zero of the job and set)
(                        it with G92 X0 Y0 Z0 first.)
(Tools:                  30 engraving bit -1/8in-. No tool change.)
(Pauses:                 The program stops 2 times with the spindle off: 'Start the spindle at)
(                        12000 rpm, then resume', 'Stop the spindle, then resume'. Resume it)
(                        when you are ready; the spindle starts again by itself.)
(Cutting:                Down to Z -1.5 mm. 2994 mm of cutting, about 7 min 40 s in all.)
(End:                    Lifts to Z 5 mm, goes to X0 Y0.)
; PutiaCNC - Marlin (no spindle control)
; Stock 100 x 100 x 12 mm; XY0 = lower-left corner of stock, Z0 = top surface
G21 G90
; Marlin: jog the tool to the zero of the job and set it there with G92 X0 Y0 Z0 before you run this file
; Spindle: by hand. The program stops to ask you to start and stop it
G0 Z5
; Tool: 30 engraving bit -1/8in-
M0 Start the spindle at 12000 rpm, then resume
; Operation 1: photo V-carve, Engraving bit 3.175 mm, lines 0.804 mm apart, up to 1.5 mm deep
G0 X21.405 Y30.4
G0 Z0.5
; Pass 1
G1 Z-0.032 F150
G1 X80 Z-1.5 F500
G0 Z1
G0 Y31.2
G0 Z0.5
; Pass 2
G1 Z-1.5 F150
G1 X21.405 Z-0.032 F500
G0 Z1
G0 Y32
G0 Z0.5
; Pass 3
G1 Z-0.032 F150
G1 X80 Z-1.5 F500
G0 Z1
G0 Y32.8
G0 Z0.5
; Pass 4
G1 Z-1.5 F150
G1 X21.405 Z-0.032 F500
G0 Z1
G0 Y33.6
G0 Z0.5
; Pass 5
G1 Z-0.032 F150
G1 X80 Z-1.5 F500
G0 Z1
G0 Y34.4
G0 Z0.5
; Pass 6
G1 Z-1.5 F150
G1 X21.405 Z-0.032 F500
G0 Z1
G0 Y35.2
G0 Z0.5
; Pass 7
G1 Z-0.032 F150
G1 X80 Z-1.5 F500
G0 Z1
G0 Y36
G0 Z0.5
; Pass 8
G1 Z-1.5 F150
G1 X21.405 Z-0.032 F500
G0 Z1
G0 Y36.8
G0 Z0.5
; Pass 9
G1 Z-0.032 F150
G1 X80 Z-1.5 F500
G0 Z1
G0 Y37.6
G0 Z0.5
; Pass 10
G1 Z-1.5 F150
G1 X21.405 Z-0.032 F500
G0 Z1
G0 Y38.4
G0 Z0.5
; Pass 11
G1 Z-0.032 F150
G1 X80 Z-1.5 F500
G0 Z1
G0 Y39.2
G0 Z0.5
; Pass 12
G1 Z-1.5 F150
G1 X21.405 Z-0.032 F500
G0 Z1
G0 Y40
G0 Z0.5
; Pass 13
G1 Z-0.032 F150
G1 X80 Z-1.5 F500
G0 Z1
G0 Y40.8
G0 Z0.5
; Pass 14
G1 Z-1.5 F150
G1 X21.405 Z-0.032 F500
G0 Z1
G0 Y41.6
G0 Z0.5
; Pass 15
G1 Z-0.032 F150
G1 X80 Z-1.5 F500
G0 Z1
G0 Y42.4
G0 Z0.5
; Pass 16
G1 Z-1.5 F150
G1 X21.405 Z-0.032 F500
G0 Z1
G0 Y43.2
G0 Z0.5
; Pass 17
G1 Z-0.032 F150
G1 X80 Z-1.5 F500
G0 Z1
G0 Y44
G0 Z0.5
; Pass 18
G1 Z-1.5 F150
G1 X21.405 Z-0.032 F500
G0 Z1
G0 Y44.8
G0 Z0.5
; Pass 19
G1 Z-0.032 F150
G1 X80 Z-1.5 F500
G0 Z1
G0 Y45.6
G0 Z0.5
; Pass 20
G1 Z-1.5 F150
G1 X21.405 Z-0.032 F500
G0 Z1
G0 Y46.4
G0 Z0.5
; Pass 21
G1 Z-0.032 F150
G1 X80 Z-1.5 F500
G0 Z1
G0 Y47.2
G0 Z0.5
; Pass 22
G1 Z-1.5 F150
G1 X21.405 Z-0.032 F500
G0 Z1
G0 Y48
G0 Z0.5
; Pass 23
G1 Z-0.032 F150
G1 X80 Z-1.5 F500
G0 Z1
G0 Y48.8
G0 Z0.5
; Pass 24
G1 Z-1.5 F150
G1 X21.405 Z-0.032 F500
G0 Z1
G0 Y49.6
G0 Z0.5
; Pass 25
G1 Z-0.032 F150
G1 X80 Z-1.5 F500
G0 Z1
G0 Y50.4
G0 Z0.5
; Pass 26
G1 Z-1.5 F150
G1 X21.405 Z-0.032 F500
G0 Z1
G0 Y51.2
G0 Z0.5
; Pass 27
G1 Z-0.032 F150
G1 X80 Z-1.5 F500
G0 Z1
G0 Y52
G0 Z0.5
; Pass 28
G1 Z-1.5 F150
G1 X21.405 Z-0.032 F500
G0 Z1
G0 Y52.8
G0 Z0.5
; Pass 29
G1 Z-0.032 F150
G1 X80 Z-1.5 F500
G0 Z1
G0 Y53.6
G0 Z0.5
; Pass 30
G1 Z-1.5 F150
G1 X21.405 Z-0.032 F500
G0 Z1
G0 Y54.4
G0 Z0.5
; Pass 31
G1 Z-0.032 F150
G1 X80 Z-1.5 F500
G0 Z1
G0 Y55.2
G0 Z0.5
; Pass 32
G1 Z-1.5 F150
G1 X21.405 Z-0.032 F500
G0 Z1
G0 Y56
G0 Z0.5
; Pass 33
G1 Z-0.032 F150
G1 X80 Z-1.5 F500
G0 Z1
G0 Y56.8
G0 Z0.5
; Pass 34
G1 Z-1.5 F150
G1 X21.405 Z-0.032 F500
G0 Z1
G0 Y57.6
G0 Z0.5
; Pass 35
G1 Z-0.032 F150
G1 X80 Z-1.5 F500
G0 Z1
G0 Y58.4
G0 Z0.5
; Pass 36
G1 Z-1.5 F150
G1 X21.405 Z-0.032 F500
G0 Z1
G0 Y59.2
G0 Z0.5
; Pass 37
G1 Z-0.032 F150
G1 X80 Z-1.5 F500
G0 Z1
G0 Y60
G0 Z0.5
; Pass 38
G1 Z-1.5 F150
G1 X21.405 Z-0.032 F500
G0 Z1
G0 Y60.8
G0 Z0.5
; Pass 39
G1 Z-0.032 F150
G1 X80 Z-1.5 F500
G0 Z1
G0 Y61.6
G0 Z0.5
; Pass 40
G1 Z-1.5 F150
G1 X21.405 Z-0.032 F500
G0 Z1
G0 Y62.4
G0 Z0.5
; Pass 41
G1 Z-0.032 F150
G1 X80 Z-1.5 F500
G0 Z1
G0 Y63.2
G0 Z0.5
; Pass 42
G1 Z-1.5 F150
G1 X21.405 Z-0.032 F500
G0 Z1
G0 Y64
G0 Z0.5
; Pass 43
G1 Z-0.032 F150
G1 X80 Z-1.5 F500
G0 Z1
G0 Y64.8
G0 Z0.5
; Pass 44
G1 Z-1.5 F150
G1 X21.405 Z-0.032 F500
G0 Z1
G0 Y65.6
G0 Z0.5
; Pass 45
G1 Z-0.032 F150
G1 X80 Z-1.5 F500
G0 Z1
G0 Y66.4
G0 Z0.5
; Pass 46
G1 Z-1.5 F150
G1 X21.405 Z-0.032 F500
G0 Z1
G0 Y67.2
G0 Z0.5
; Pass 47
G1 Z-0.032 F150
G1 X80 Z-1.5 F500
G0 Z1
G0 Y68
G0 Z0.5
; Pass 48
G1 Z-1.5 F150
G1 X21.405 Z-0.032 F500
G0 Z1
G0 Y68.8
G0 Z0.5
; Pass 49
G1 Z-0.032 F150
G1 X80 Z-1.5 F500
G0 Z1
G0 Y69.6
G0 Z0.5
; Pass 50
G1 Z-1.5 F150
G1 X21.405 Z-0.032 F500
G0 Z5
M0 Stop the spindle, then resume
G0 X0 Y0
; End of program