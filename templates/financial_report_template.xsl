<xsl:stylesheet version="1.0"
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:fo="http://www.w3.org/1999/XSL/Format">

    <!-- Page Layout -->
    <fo:simple-page-master page-width="8.5in" page-height="11in" margin="50pt"/>

    <!-- ============================================= -->
    <!-- TEMPLATES for each data row type (using match)-->
    <!-- ============================================= -->
    <!-- This is the idiomatic XSLT way to handle different data structures. -->
    <!-- It's more modular and extensible than xsl:if or xsl:choose. -->

    <xsl:template match="data[type='item']">
        <row>
            <cell use-attribute-sets="td"><text><xsl:value-of select="label"/></text></cell>
            <cell use-attribute-sets="td td-right"><text>{{formatCurrency value}}</text></cell>
        </row>
    </xsl:template>

    <xsl:template match="data[type='subtotal']">
        <row>
            <cell use-attribute-sets="td subtotal-row-cell"><text><xsl:value-of select="label"/></text></cell>
            <cell use-attribute-sets="td td-right subtotal-row-cell"><text>{{formatCurrency value}}</text></cell>
        </row>
    </xsl:template>

    <xsl:template match="data[type='total']">
        <row>
            <cell use-attribute-sets="td total-row-cell"><text><xsl:value-of select="label"/></text></cell>
            <cell use-attribute-sets="td td-right total-row-cell"><text>{{formatCurrency value}}</text></cell>
        </row>
    </xsl:template>

    <!-- ============================================= -->
    <!-- Main Template (Entry Point)                   -->
    <!-- ============================================= -->
    <xsl:template match="/">
        <page-sequence select=".">
            <text use-attribute-sets="h1"><xsl:value-of select="companyName"/></text>
            <text use-attribute-sets="h2"><xsl:value-of select="reportTitle"/></text>

            <xsl:for-each select="sections">
                <text use-attribute-sets="section-title"><xsl:value-of select="title"/></text>

                <xsl:for-each select="tables">
                    <xsl:if test="title">
                        <text use-attribute-sets="table-title"><xsl:value-of select="title"/></text>
                    </xsl:if>

                    <table>
                        <columns>
                            <column width="70%"/>
                            <column width="30%"/>
                        </columns>
                        <tbody>
                            <!--
                              *** CHANGE IS HERE ***
                              apply-templates will automatically find the best matching
                              template rule for each `data` element based on its type.
                            -->
                            <xsl:apply-templates select="data"/>
                        </tbody>
                    </table>
                </xsl:for-each>
            </xsl:for-each>
        </page-sequence>
    </xsl:template>

    <!-- ============================================= -->
    <!-- Style Definitions                             -->
    <!-- ============================================= -->
    <xsl:attribute-set name="h1">
        <xsl:attribute name="font-size">32pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="color">#142850</xsl:attribute>
        <xsl:attribute name="margin-bottom">10pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="h2">
        <xsl:attribute name="font-size">24pt</xsl:attribute>
        <xsl:attribute name="color">#27496d</xsl:attribute>
        <xsl:attribute name="margin-bottom">40pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="section-title">
        <xsl:attribute name="font-size">18pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="color">#142850</xsl:attribute>
        <xsl:attribute name="border-bottom">1pt solid #142850</xsl:attribute>
        <xsl:attribute name="padding-bottom">4pt</xsl:attribute>
        <xsl:attribute name="margin-top">20pt</xsl:attribute>
        <xsl:attribute name="margin-bottom">15pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="table-title">
        <xsl:attribute name="font-size">14pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="margin-bottom">10pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="td">
        <xsl:attribute name="font-size">11pt</xsl:attribute>
        <xsl:attribute name="padding-top">6pt</xsl:attribute>
        <xsl:attribute name="padding-bottom">6pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="td-right" use-attribute-sets="td">
        <xsl:attribute name="text-align">right</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="subtotal-row-cell">
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="border-top">0.5pt solid #888888</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="total-row-cell">
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="border-top">1.5pt solid #142850</xsl:attribute>
    </xsl:attribute-set>

</xsl:stylesheet>