(Summary, written by PutiaCNC)
(Zero:                   XY0 = lower-left corner of stock, Z0 = top surface, in G54)
(Start:                  Starts the spindle at 12000 rpm and waits 2 s, then moves to X46.681)
(                        Y43.32.)
(Tools:                  60 V-bit -1/4in-, 1/8in flat end mill. The program pauses 1 time for)
(                        you to change the tool -to 6.35 mm-.)
(At a tool change (!):   Touch off Z0 again with the new tool before you resume: the controller)
(                        does not know how long each tool is.)
(Cutting:                Down to Z -3 mm. 613 mm of cutting, about 1 min 12 s in all.)
(End:                    Lifts to Z 5 mm, stops the spindle, goes to X0 Y0.)
(PutiaCNC - grblHAL)
(Stock 200 x 150 x 12 mm; XY0 = lower-left corner of stock, Z0 = top surface)
G21 G90 G17 G94 G54
G0 Z5
(Tool: 1/8in flat end mill)
M3 S12000
G4 P2
(Operation 1: inlay pocket, clear the floor, flat 3.175 mm)
G0 X46.681 Y43.32
G0 Z0.5
G1 Z0 F250
(Pass 1)
G1 F800
G1 X33.32 Z-0.4
G1 Y46.681 Z-0.5
G1 X46.681 Z-0.9
G1 Y43.32 Z-1
(Pass 1, levelling lap)
G1 X33.32
G1 Y46.681
G1 X46.681
G1 Y43.32
G0 Z5
G0 X45.411 Y44.59
G0 Z0.5
G1 Z0 F250
(Pass 1)
G1 Z-1
G1 F800
G1 X34.59
G1 Y45.411
G1 X45.411
G1 Y44.59
G0 Z5
G0 X46.681 Y43.32
G0 Z0.5
G1 Z0 F250
G1 Z-1
(Pass 2)
G1 F800
G1 X33.32 Z-1.4
G1 Y46.681 Z-1.5
G1 X46.681 Z-1.9
G1 Y43.32 Z-2
(Pass 2, levelling lap)
G1 X33.32
G1 Y46.681
G1 X46.681
G1 Y43.32
G0 Z5
G0 X45.411 Y44.59
G0 Z0.5
G1 Z0 F250
G1 Z-1
(Pass 2)
G1 Z-2
G1 F800
G1 X34.59
G1 Y45.411
G1 X45.411
G1 Y44.59
G0 Z5
G0 X46.681 Y43.32
G0 Z0.5
G1 Z0 F250
G1 Z-2
(Pass 3)
G1 F800
G1 X33.32 Z-2.4
G1 Y46.681 Z-2.5
G1 X46.681 Z-2.9
G1 Y43.32 Z-3
(Pass 3, levelling lap)
G1 X33.32
G1 Y46.681
G1 X46.681
G1 Y43.32
G0 Z5
G0 X45.411 Y44.59
G0 Z0.5
G1 Z0 F250
G1 Z-2
(Pass 3)
G1 Z-3
G1 F800
G1 X34.59
G1 Y45.411
G1 X45.411
G1 Y44.59
G0 Z5
M5
M0 ; Change tool to 6.35 mm, then touch off Z0 and resume
(Tool: 60 V-bit -1/4in-)
M3 S12000
G4 P2
(Operation 1: inlay pocket, sloped walls, 60 deg V-bit 6.35 mm)
G0 X49.7 Y40.3
(Pass 1)
G1 Z-0.52 F250
G1 F800
G1 X30.3
G1 Y49.7
G1 X49.7
G1 Y40.3
G0 Z1
G0 X49.4 Y40.6
(Pass 2)
G1 Z-1.039 F250
G1 F800
G1 X30.6
G1 Y49.4
G1 X49.4
G1 Y40.6
G0 Z1
G0 X49.1 Y40.9
(Pass 3)
G1 Z-1.559 F250
G1 F800
G1 X30.9
G1 Y49.1
G1 X49.1
G1 Y40.9
G0 Z1
G0 X48.8 Y41.2
(Pass 4)
G1 Z-2.078 F250
G1 F800
G1 X31.2
G1 Y48.8
G1 X48.8
G1 Y41.2
G0 Z1
G0 X48.5 Y41.5
(Pass 5)
G1 Z-2.598 F250
G1 F800
G1 X31.5
G1 Y48.5
G1 X48.5
G1 Y41.5
G0 Z1
G0 X48.268 Y41.732
(Pass 6)
G1 Z-3 F250
G1 F800
G1 X31.732
G1 Y48.268
G1 X48.268
G1 Y41.732
G0 Z5
M5
G0 X0 Y0
M2