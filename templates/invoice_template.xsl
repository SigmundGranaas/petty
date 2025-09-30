<xsl:stylesheet version="1.0"
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:fo="http://www.w3.org/1999/XSL/Format">

    <!-- The simple-page-master is the modern, preferred way to define page layout. -->
    <fo:simple-page-master master-name="A4"
                           page-width="210mm" page-height="297mm"
                           margin="20mm">
    </fo:simple-page-master>

    <!--
      This named template defines the body for a single invoice.
      It can be reused or called from different places.
    -->
    <xsl:template name="invoice-body">
        <text use-attribute-sets="h1">Invoice <xsl:value-of select="invoiceNumber"/></text>
        <text use-attribute-sets="h2"><xsl:value-of select="name"/></text>
        <text><xsl:value-of select="address"/></text>

        <table use-attribute-sets="invoice-table">
            <columns>
                <column width="50%" header-style="th" style="td"/>
                <column width="25%" header-style="th-right" style="td-right"/>
                <column width="25%" header-style="th-right" style="td-right"/>
            </columns>
            <header>
                <row>
                    <cell><text>Product</text></cell>
                    <cell><text>Quantity</text></cell>
                    <cell><text>Price</text></cell>
                </row>
            </header>
            <tbody>
                <xsl:for-each select="items">
                    <row>
                        <cell><text><xsl:value-of select="product"/></text></cell>
                        <cell><text><xsl:value-of select="quantity"/></text></cell>
                        <cell><text>{{formatCurrency price}}</text></cell>
                    </row>
                </xsl:for-each>
            </tbody>
        </table>
    </xsl:template>

    <!--
      The root template defines the document structure.
      The <page-sequence> tag indicates that a new document sequence
      should be created for each item in `customers`.
    -->
    <xsl:template match="/">
        <page-sequence select="customers">
            <!--
              For each customer, we call the named template to render the invoice body.
              The context (.) inside the called template will be the customer object.
            -->
            <xsl:call-template name="invoice-body"/>
        </page-sequence>
    </xsl:template>

    <!-- Styles -->
    <xsl:attribute-set name="h1">
        <xsl:attribute name="font-size">24pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="margin-bottom">12pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="h2">
        <xsl:attribute name="font-size">16pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="margin-bottom">4pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="invoice-table">
        <xsl:attribute name="margin-top">20pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="th">
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="padding">4pt</xsl:attribute>
        <xsl:attribute name="background-color">#EEEEEE</xsl:attribute>
    </xsl:attribute-set>

    <!-- More efficient to compose styles this way. -->
    <xsl:attribute-set name="th-right" use-attribute-sets="th">
        <xsl:attribute name="text-align">right</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="td">
        <xsl:attribute name="padding">4pt</xsl:attribute>
        <xsl:attribute name="border-bottom">1pt solid #CCCCCC</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="td-right" use-attribute-sets="td">
        <xsl:attribute name="text-align">right</xsl:attribute>
    </xsl:attribute-set>

</xsl:stylesheet>